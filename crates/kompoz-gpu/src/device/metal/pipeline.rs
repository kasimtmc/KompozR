//! Pipelines: made from the shaders' MSL (each library compiled once, specialized through function constants), on
//! any thread ([`Maker`] is the device's [`PipelineFactory`]: Metal's device, libraries, functions and pipeline states
//! are thread-safe), and kept between runs in a binary archive ([`PipelineCache`]).
//!
//! The binary archive is Metal's pipeline cache: the GPU's binaries of the pipelines made, which a pipeline made with
//! the archive takes instead of compiling them again. It is serialized into a file of Metal's own, which the device
//! keeps inside the pipeline cache file every backend has (`KPZPIPE2`, whole or not at all: [`write_pipeline_cache`])
//! behind a header naming the GPU and the system it was compiled for (the archive of another GPU or system is passed
//! over, as Vulkan's driver passes over another's cache by its header). A pipeline is first asked of the archive
//! alone (`FailOnBinaryArchiveMiss`), so that the device knows when it compiled one: only then is the archive written
//! again, at a frame's end. (Compiling a library's MSL into Metal's intermediate code is cached by the system itself,
//! per user, between runs.)

use super::{describe, mtl_format};
use crate::device::{
    BlendFactor, BlendOp, Format, Pipeline, PipelineDesc, PipelineFactory, PipelineId, RetiredQueue, ShaderCode,
    VertexFormat, log, read_pipeline_cache, write_pipeline_cache,
};
use objc2::available;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSArray, NSProcessInfo, NSString, NSURL};
use objc2_metal::{
    MTLBinaryArchive, MTLBinaryArchiveDescriptor, MTLBlendFactor, MTLBlendOperation, MTLCompileOptions, MTLDataType,
    MTLDevice, MTLFunction, MTLFunctionConstantValues, MTLLibrary, MTLMathMode, MTLPipelineOption,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLVertexDescriptor, MTLVertexFormat, MTLVertexStepFunction,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The device's own pipeline (not the renderer's): what clears part of a target, where a pass with an area clears
/// only there (a render pass's clear is all of its target's on Metal; Vulkan's render area clears only within it).
/// One triangle over all of clip space, cut by the pass's scissor, its color at buffer 0.
const CLEAR_MSL: &str = "#include <metal_stdlib>
using namespace metal;
struct Cleared { float4 position [[position]]; };
vertex Cleared kompoz_clear_vs(uint vertex_id [[vertex_id]]) {
    float2 corner = float2(float((vertex_id << 1) & 2), float(vertex_id & 2));
    return Cleared { float4(corner * 2.0 - 1.0, 0.0, 1.0) };
}
fragment float4 kompoz_clear_fs(constant float4& color [[buffer(0)]]) { return color; }
";

/// The clear pipeline's shaders (no SPIR-V or WGSL: Metal's only).
const CLEAR_VERTEX: ShaderCode = ShaderCode { spirv: &[], msl: CLEAR_MSL, wgsl: "", entry: "kompoz_clear_vs" };
const CLEAR_FRAGMENT: ShaderCode = ShaderCode { spirv: &[], msl: CLEAR_MSL, wgsl: "", entry: "kompoz_clear_fs" };

fn mtl_vertex_format(format: VertexFormat) -> MTLVertexFormat {
    match format {
        VertexFormat::Float => MTLVertexFormat::Float,
        VertexFormat::Float2 => MTLVertexFormat::Float2,
        VertexFormat::Float3 => MTLVertexFormat::Float3,
        VertexFormat::Float4 => MTLVertexFormat::Float4,
        VertexFormat::UInt4 => MTLVertexFormat::UInt4,
    }
}

fn mtl_factor(factor: BlendFactor) -> MTLBlendFactor {
    match factor {
        BlendFactor::Zero => MTLBlendFactor::Zero,
        BlendFactor::One => MTLBlendFactor::One,
        BlendFactor::SrcAlpha => MTLBlendFactor::SourceAlpha,
        BlendFactor::OneMinusSrcAlpha => MTLBlendFactor::OneMinusSourceAlpha,
        BlendFactor::DstAlpha => MTLBlendFactor::DestinationAlpha,
        BlendFactor::OneMinusDstAlpha => MTLBlendFactor::OneMinusDestinationAlpha,
        BlendFactor::SrcColor => MTLBlendFactor::SourceColor,
        BlendFactor::OneMinusSrcColor => MTLBlendFactor::OneMinusSourceColor,
        BlendFactor::DstColor => MTLBlendFactor::DestinationColor,
        BlendFactor::OneMinusDstColor => MTLBlendFactor::OneMinusDestinationColor,
    }
}

fn mtl_op(op: BlendOp) -> MTLBlendOperation {
    match op {
        BlendOp::Add => MTLBlendOperation::Add,
        BlendOp::Subtract => MTLBlendOperation::Subtract,
        BlendOp::ReverseSubtract => MTLBlendOperation::ReverseSubtract,
        BlendOp::Min => MTLBlendOperation::Min,
        BlendOp::Max => MTLBlendOperation::Max,
    }
}

/// The binary archive, used from the threads pipelines are made on.
struct Archive(Retained<ProtocolObject<dyn MTLBinaryArchive>>);

// SAFETY: Metal's objects other than command buffers and encoders may be used from any thread (Apple's "Metal
// objects… are thread-safe" for devices, libraries, pipeline states and archives); objc2 leaves the archive's protocol
// without Send and Sync only because its header doesn't say so. What changes it (adding pipelines, serializing) is
// done under PipelineCache's lock.
unsafe impl Send for Archive {}
// SAFETY: as above.
unsafe impl Sync for Archive {}

/// The pipelines kept between runs: the binary archive pipelines are made with, and its file.
pub(super) struct PipelineCache {
    file: PathBuf,
    /// What the archive's data starts with in the file: this GPU's and system's.
    header: Vec<u8>,
    archive: Archive,
    /// The archive's own file it was read from (Metal reads it as it needs it): removed with the device.
    loaded: Option<PathBuf>,
    /// Pipelines were compiled that the archive didn't have: it is written again.
    made: AtomicBool,
    lock: Mutex<()>,
}

/// A file beside `file` of its own (another run of the app has its own, and so has another device in this one).
fn beside(file: &Path, what: &str) -> PathBuf {
    static FILES: AtomicU32 = AtomicU32::new(0);
    let n = FILES.fetch_add(1, Ordering::Relaxed);
    let mut name = file.as_os_str().to_owned();
    name.push(format!(".{}.{n}.{what}.metalar", std::process::id()));
    PathBuf::from(name)
}

fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

impl PipelineCache {
    /// The archive kept in `file` if it is this GPU's and system's, else a new one; none if Metal can't make one.
    pub fn open(device: &ProtocolObject<dyn MTLDevice>, file: PathBuf) -> Option<PipelineCache> {
        let mut header = b"KPZMTLA1".to_vec();
        let os = NSProcessInfo::processInfo().operatingSystemVersionString().to_string();
        for part in [device.name().to_string(), os] {
            header.extend_from_slice(&(part.len() as u32).to_le_bytes());
            header.extend_from_slice(part.as_bytes());
        }
        let kept = read_pipeline_cache(&file);
        let mut loaded = None;
        if kept.len() > header.len() && kept.starts_with(&header) {
            let path = beside(&file, "kept");
            if std::fs::write(&path, &kept[header.len()..]).is_ok() {
                loaded = Some(path);
            }
        }
        let made = |from: Option<&Path>| {
            let descriptor = MTLBinaryArchiveDescriptor::new();
            if let Some(from) = from {
                descriptor.setUrl(Some(&file_url(from)));
            }
            device.newBinaryArchiveWithDescriptor_error(&descriptor)
        };
        let archive = match made(loaded.as_deref()) {
            Ok(archive) => archive,
            Err(error) => {
                if let Some(path) = loaded.take() {
                    log(&format!("Metal: the kept pipelines couldn't be read: {}", describe(Some(&error))));
                    let _ = std::fs::remove_file(path);
                }
                match made(None) {
                    Ok(archive) => archive,
                    Err(error) => {
                        log(&format!("Metal: no binary archive for the pipelines: {}", describe(Some(&error))));
                        return None;
                    }
                }
            }
        };
        Some(PipelineCache {
            file,
            header,
            archive: Archive(archive),
            loaded,
            made: AtomicBool::new(false),
            lock: Mutex::new(()),
        })
    }

    /// The pipeline's binaries into the archive (compiled for it).
    fn add(&self, descriptor: &MTLRenderPipelineDescriptor) {
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        match self.archive.0.addRenderPipelineFunctionsWithDescriptor_error(descriptor) {
            Ok(()) => self.made.store(true, Ordering::Relaxed),
            Err(error) => log(&format!("Metal: a pipeline wasn't kept: {}", describe(Some(&error)))),
        }
    }

    /// The archive into its file, if pipelines were added to it.
    fn keep(&self) {
        if !self.made.swap(false, Ordering::Relaxed) {
            return;
        }
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let out = beside(&self.file, "out");
        // (Metal writes the archive only into a directory there is — "Output URL is invalid" — and the cache's isn't
        // yet in the first run: it is made here, as writing the cache would.)
        if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&out);
        match self.archive.0.serializeToURL_error(&file_url(&out)) {
            Ok(()) => match std::fs::read(&out) {
                Ok(bytes) => {
                    let mut data = self.header.clone();
                    data.extend_from_slice(&bytes);
                    write_pipeline_cache(&self.file, &data);
                }
                Err(_) => log("Metal: the pipelines' archive couldn't be read back"),
            },
            Err(error) => log(&format!("Metal: the pipelines' archive wasn't written: {}", describe(Some(&error)))),
        }
        let _ = std::fs::remove_file(&out);
    }
}

impl Drop for PipelineCache {
    fn drop(&mut self) {
        if let Some(path) = self.loaded.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Makes pipelines, on the device's thread and others, and holds them by id.
pub(super) struct Maker {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    /// Each shader file's library, by its MSL (each compiled once).
    libraries: Mutex<Vec<(usize, Retained<ProtocolObject<dyn MTLLibrary>>)>>,
    pipelines: Mutex<HashMap<u64, Retained<ProtocolObject<dyn MTLRenderPipelineState>>>>,
    next: AtomicU64,
    cache: Option<PipelineCache>,
    errors: Arc<AtomicU32>,
    retired: Arc<RetiredQueue>,
}

impl Maker {
    pub fn new(
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        cache: Option<PipelineCache>,
        errors: Arc<AtomicU32>,
        retired: Arc<RetiredQueue>,
    ) -> Maker {
        Maker {
            device,
            libraries: Mutex::new(Vec::new()),
            pipelines: Mutex::new(HashMap::new()),
            next: AtomicU64::new(0),
            cache,
            errors,
            retired,
        }
    }

    fn failed(&self, what: &str, error: Option<&objc2_foundation::NSError>) {
        log(&format!("Metal: {what}: {}", describe(error)));
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// The library of a shader file's MSL, compiled the first time (without fast math: the same arithmetic as the
    /// other backends).
    fn library(&self, msl: &'static str) -> Option<Retained<ProtocolObject<dyn MTLLibrary>>> {
        let key = msl.as_ptr() as usize;
        // (Held while compiling: pipelines made at once on other threads from the same file wait for its one library.)
        let mut libraries = self.libraries.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, library)) = libraries.iter().find(|(k, _)| *k == key) {
            return Some(library.clone());
        }
        let options = MTLCompileOptions::new();
        if available!(macos = 15.0, ios = 18.0, tvos = 18.0, visionos = 2.0) {
            options.setMathMode(MTLMathMode::Safe);
        } else {
            #[allow(deprecated)]
            options.setFastMathEnabled(false);
        }
        match self.device.newLibraryWithSource_options_error(&NSString::from_str(msl), Some(&options)) {
            Ok(library) => {
                libraries.push((key, library.clone()));
                Some(library)
            }
            Err(error) => {
                self.failed("a shader didn't compile", Some(&error));
                None
            }
        }
    }

    /// The shader's function, specialized by `constants` (its function constants 0, 1, …; a function with constants
    /// is made through them even with none given: those keep their defaults).
    fn function(&self, code: &ShaderCode, constants: &[i32]) -> Option<Retained<ProtocolObject<dyn MTLFunction>>> {
        let library = self.library(code.msl)?;
        let name = NSString::from_str(code.entry);
        let Some(plain) = library.newFunctionWithName(&name) else {
            self.failed(&format!("no function {} in its shader", code.entry), None);
            return None;
        };
        if plain.functionConstantsDictionary().count() == 0 {
            return Some(plain);
        }
        let values = MTLFunctionConstantValues::new();
        for (i, constant) in constants.iter().enumerate() {
            // SAFETY: the value is an int, as the type says, read during the call.
            unsafe { values.setConstantValue_type_atIndex(NonNull::from(constant).cast(), MTLDataType::Int, i) };
        }
        match library.newFunctionWithName_constantValues_error(&name, &values) {
            Ok(function) => Some(function),
            Err(error) => {
                self.failed("a shader wasn't specialized", Some(&error));
                None
            }
        }
    }

    /// A pipeline state for `desc`; with the archive, from its binaries if it has them (else compiled and added).
    fn state(
        &self,
        desc: &PipelineDesc<'_>,
        cached: bool,
    ) -> Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        let vertex = self.function(desc.vertex, &[])?;
        let fragment = self.function(desc.fragment, &desc.constants)?;
        let d = MTLRenderPipelineDescriptor::new();
        d.setVertexFunction(Some(&vertex));
        d.setFragmentFunction(Some(&fragment));
        if !desc.instance_attributes.is_empty() {
            // The instances' attributes from buffer 1 (buffer 0: the uniforms), one step an instance.
            let vd = MTLVertexDescriptor::vertexDescriptor();
            for a in &desc.instance_attributes {
                assert!(a.location < 31, "an attribute's location past Metal's 31");
                // SAFETY: the location is within Metal's attributes (31).
                let attribute = unsafe { vd.attributes().objectAtIndexedSubscript(a.location as usize) };
                attribute.setFormat(mtl_vertex_format(a.format));
                // SAFETY: offsets within the instance (the renderer's layouts), buffer 1 one of Metal's 31.
                unsafe {
                    attribute.setOffset(a.offset as usize);
                    attribute.setBufferIndex(1);
                }
            }
            // SAFETY: buffer 1 is within Metal's buffers (31).
            let layout = unsafe { vd.layouts().objectAtIndexedSubscript(1) };
            // SAFETY: the instance's size, a multiple of 4 (its attributes' floats and ints).
            unsafe { layout.setStride(desc.instance_stride as usize) };
            layout.setStepFunction(MTLVertexStepFunction::PerInstance);
            d.setVertexDescriptor(Some(&vd));
        }
        // SAFETY: attachment 0, within Metal's 8.
        let color = unsafe { d.colorAttachments().objectAtIndexedSubscript(0) };
        color.setPixelFormat(mtl_format(desc.target));
        color.setBlendingEnabled(desc.blend.enabled);
        color.setSourceRGBBlendFactor(mtl_factor(desc.blend.src_color));
        color.setDestinationRGBBlendFactor(mtl_factor(desc.blend.dst_color));
        color.setSourceAlphaBlendFactor(mtl_factor(desc.blend.src_alpha));
        color.setDestinationAlphaBlendFactor(mtl_factor(desc.blend.dst_alpha));
        color.setRgbBlendOperation(mtl_op(desc.blend.color_op));
        color.setAlphaBlendOperation(mtl_op(desc.blend.alpha_op));
        let cache = self.cache.as_ref().filter(|_| cached);
        if let Some(cache) = cache {
            d.setBinaryArchives(Some(&NSArray::from_retained_slice(std::slice::from_ref(&cache.archive.0))));
            let options = MTLPipelineOption::FailOnBinaryArchiveMiss;
            if let Ok(state) =
                self.device.newRenderPipelineStateWithDescriptor_options_reflection_error(&d, options, None)
            {
                return Some(state);
            }
            // (Not in the archive: compiled into it, then made from it.)
            cache.add(&d);
        }
        match self.device.newRenderPipelineStateWithDescriptor_error(&d) {
            Ok(state) => Some(state),
            Err(error) => {
                self.failed("a pipeline wasn't made", Some(&error));
                None
            }
        }
    }

    /// A pipeline for `desc`, by id.
    pub fn make(&self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        autoreleasepool(|_| {
            let state = self.state(desc, true)?;
            let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
            self.pipelines.lock().unwrap_or_else(|e| e.into_inner()).insert(id, state);
            Some(Pipeline::new(PipelineId::new(id), self.retired.clone()))
        })
    }

    /// The device's pipeline clearing part of a `format` target (not kept in the archive: the renderer's are what
    /// it is for).
    pub fn clear_pipeline(&self, format: Format) -> Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        let mut desc = PipelineDesc::new(&CLEAR_VERTEX, &CLEAR_FRAGMENT);
        desc.blend = crate::device::BlendState::OFF;
        desc.target = format;
        self.state(&desc, false)
    }

    /// The pipeline state of an id (none if it is gone).
    pub fn pipeline_state(&self, id: PipelineId) -> Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        self.pipelines.lock().unwrap_or_else(|e| e.into_inner()).get(&id.raw()).cloned()
    }

    /// The pipeline out of the table (destroyed once the GPU is done with the frames that may use it).
    pub fn remove(&self, id: PipelineId) -> Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        self.pipelines.lock().unwrap_or_else(|e| e.into_inner()).remove(&id.raw())
    }

    /// The archive into its file, if pipelines it didn't have were made (at a frame's end).
    pub fn keep_pipelines(&self) {
        if let Some(cache) = &self.cache {
            autoreleasepool(|_| cache.keep());
        }
    }
}

impl PipelineFactory for Maker {
    fn create_pipeline(&self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        self.make(desc)
    }
}
