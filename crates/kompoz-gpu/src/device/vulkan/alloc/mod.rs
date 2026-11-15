//! The device's memory, our own (what AMD's Vulkan Memory Allocator did for the C++ Kompoz): a memory type chosen
//! for what a resource is for, 4 MB blocks per memory type and tiling shared by the resources that fit (first fit,
//! free ranges merging again), a memory of its own for a resource over half a block or whose driver prefers it so,
//! host-visible memories mapped once for good, an emptied block kept as a spare per type, and the device's count of
//! memories kept to. The bookkeeping is [`core`], safe and tested apart from the API (under Miri too); this module
//! carries it out on Vulkan.

pub mod core;

pub use self::core::{Allocation, MemoryUsage, Stats, Tiling};

use self::core::{AllocatorCore, Request, Step, memory_type_candidates};
use ash::vk;
use std::ptr::NonNull;

/// A device memory: the handle, and where it is mapped (host-visible ones, for good).
pub struct DeviceMemory {
    memory: vk::DeviceMemory,
    mapped: Option<NonNull<u8>>,
}

/// A resource to give memory to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Image(vk::Image),
    Buffer(vk::Buffer),
}

/// The device's memory and what is in it.
pub struct GpuAllocator {
    core: AllocatorCore<DeviceMemory>,
    types: Vec<vk::MemoryType>,
    heaps: Vec<vk::MemoryHeap>,
    non_coherent_atom: u64,
}

impl GpuAllocator {
    /// For the device made on `physical` (with these properties).
    pub fn new(
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
        properties: &vk::PhysicalDeviceProperties,
    ) -> GpuAllocator {
        // SAFETY: `physical` is one of `instance`'s physical devices.
        let memory = unsafe { instance.get_physical_device_memory_properties(physical) };
        GpuAllocator {
            core: AllocatorCore::new(properties.limits.max_memory_allocation_count as usize),
            types: memory.memory_types_as_slice().to_vec(),
            heaps: memory.memory_heaps_as_slice().to_vec(),
            non_coherent_atom: properties.limits.non_coherent_atom_size.max(1),
        }
    }

    /// Memory for `resource` (not bound to any yet) in the best type for `usage` that has room, bound to it.
    ///
    /// # Safety
    /// `device` is the device this allocator was made for, and `resource` one of its, alive and without memory.
    pub unsafe fn allocate(
        &mut self,
        device: &ash::Device,
        resource: Resource,
        usage: MemoryUsage,
    ) -> Result<Allocation, vk::Result> {
        let mut dedicated_requirements = vk::MemoryDedicatedRequirements::default();
        let mut requirements2 = vk::MemoryRequirements2::default().push_next(&mut dedicated_requirements);
        // SAFETY: the resource is the device's (the caller's promise); the structures chain as Vulkan 1.1 has them.
        unsafe {
            match resource {
                Resource::Image(image) => device.get_image_memory_requirements2(
                    &vk::ImageMemoryRequirementsInfo2::default().image(image),
                    &mut requirements2,
                ),
                Resource::Buffer(buffer) => device.get_buffer_memory_requirements2(
                    &vk::BufferMemoryRequirementsInfo2::default().buffer(buffer),
                    &mut requirements2,
                ),
            }
        }
        let requirements = requirements2.memory_requirements;
        let dedicated = dedicated_requirements.prefers_dedicated_allocation == vk::TRUE
            || dedicated_requirements.requires_dedicated_allocation == vk::TRUE;
        let tiling = match resource {
            Resource::Image(_) => Tiling::Optimal,
            Resource::Buffer(_) => Tiling::Linear,
        };
        let mut failure = vk::Result::ERROR_OUT_OF_DEVICE_MEMORY;
        for memory_type in memory_type_candidates(&self.types, &self.heaps, requirements.memory_type_bits, usage) {
            let flags = self.types[memory_type as usize].property_flags;
            let (mut size, mut alignment) = (requirements.size, requirements.alignment);
            if flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE)
                && !flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT)
            {
                // Invalidated in whole atoms, which nothing else shares.
                alignment = alignment.max(self.non_coherent_atom);
                size = size.div_ceil(self.non_coherent_atom) * self.non_coherent_atom;
            }
            let request = Request { size, alignment, memory_type, tiling, dedicated };
            let allocation = match self.core.allocate(&request) {
                Step::Placed(allocation) => allocation,
                Step::NeedsMemory { size, dedicated, release } => {
                    for memory in release {
                        // SAFETY: a spare block: nothing is in it.
                        unsafe { release_memory(device, memory) };
                    }
                    // SAFETY: the caller's promise.
                    match unsafe { self.make_memory(device, &request, size, dedicated, resource) } {
                        Ok((memory, size)) => self.core.add_memory(&request, size, dedicated, memory),
                        Err(e) => {
                            failure = e;
                            continue;
                        }
                    }
                }
                Step::OutOfMemories => return Err(vk::Result::ERROR_TOO_MANY_OBJECTS),
            };
            let memory = self.core.payload(&allocation).memory;
            // SAFETY: the memory is the device's, the place in it the resource's alone, aligned as it requires.
            let bound = unsafe {
                match resource {
                    Resource::Image(image) => device.bind_image_memory(image, memory, allocation.offset()),
                    Resource::Buffer(buffer) => device.bind_buffer_memory(buffer, memory, allocation.offset()),
                }
            };
            if let Err(e) = bound {
                // SAFETY: nothing was bound to it.
                unsafe { self.free(device, allocation) };
                return Err(e);
            }
            return Ok(allocation);
        }
        Err(failure)
    }

    /// A device memory of `size` bytes in the request's type (a block smaller by halves where the device has no
    /// more, as long as the request fits), mapped if host-visible, and its size.
    ///
    /// # Safety
    /// As [`allocate`](GpuAllocator::allocate)'s.
    unsafe fn make_memory(
        &self,
        device: &ash::Device,
        request: &Request,
        size: u64,
        dedicated: bool,
        resource: Resource,
    ) -> Result<(DeviceMemory, u64), vk::Result> {
        let sizes: Vec<u64> = if dedicated {
            vec![size]
        } else {
            (0..4).map(|halvings| size >> halvings).filter(|&s| s >= request.size).collect()
        };
        let mut failure = vk::Result::ERROR_OUT_OF_DEVICE_MEMORY;
        for size in sizes {
            let mut for_resource = match resource {
                Resource::Image(image) => vk::MemoryDedicatedAllocateInfo::default().image(image),
                Resource::Buffer(buffer) => vk::MemoryDedicatedAllocateInfo::default().buffer(buffer),
            };
            let mut info =
                vk::MemoryAllocateInfo::default().allocation_size(size).memory_type_index(request.memory_type);
            if dedicated {
                info = info.push_next(&mut for_resource);
            }
            // SAFETY: a valid allocation info for the device (a dedicated one naming the caller's resource).
            let memory = match unsafe { device.allocate_memory(&info, None) } {
                Ok(memory) => memory,
                Err(e) => {
                    failure = e;
                    continue;
                }
            };
            let host_visible =
                self.types[request.memory_type as usize].property_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE);
            let mapped = if host_visible {
                // SAFETY: the memory was just made, host-visible and not mapped.
                match unsafe { device.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty()) } {
                    Ok(pointer) => NonNull::new(pointer.cast::<u8>()),
                    Err(e) => {
                        // SAFETY: nothing uses it.
                        unsafe { device.free_memory(memory, None) };
                        return Err(e);
                    }
                }
            } else {
                None
            };
            return Ok((DeviceMemory { memory, mapped }, size));
        }
        Err(failure)
    }

    /// Where the CPU reads and writes the allocation (host-visible memory's), for its size.
    pub fn mapped(&self, allocation: &Allocation) -> Option<NonNull<u8>> {
        let base = self.core.payload(allocation).mapped?;
        NonNull::new(base.as_ptr().wrapping_add(allocation.offset() as usize))
    }

    /// What the GPU wrote into the allocation made visible to the CPU (where its memory isn't coherent).
    ///
    /// # Safety
    /// `device` is the device this allocator was made for, and the GPU is done writing the allocation.
    pub unsafe fn invalidate(&self, device: &ash::Device, allocation: &Allocation) -> Result<(), vk::Result> {
        let flags = self.types[allocation.memory_type() as usize].property_flags;
        if flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {
            return Ok(());
        }
        // (In whole atoms: such allocations start at one and are made of them.)
        let range = vk::MappedMemoryRange::default()
            .memory(self.core.payload(allocation).memory)
            .offset(allocation.offset())
            .size(allocation.size());
        // SAFETY: the memory is mapped (it is host-visible), the range within it and in whole atoms.
        unsafe { device.invalidate_mapped_memory_ranges(&[range]) }
    }

    /// The allocation's place free again (its memory given back if nothing is left in it but a spare block).
    ///
    /// # Safety
    /// `device` is the device this allocator was made for, and what was bound to the allocation is destroyed (or the
    /// GPU is done with it and it is destroyed before anything else is bound there).
    pub unsafe fn free(&mut self, device: &ash::Device, allocation: Allocation) {
        if let Some(memory) = self.core.free(allocation) {
            // SAFETY: nothing is in it any more (the caller's promise for this allocation; the rest freed before).
            unsafe { release_memory(device, memory) };
        }
    }

    /// Every memory given back, at the end.
    ///
    /// # Safety
    /// `device` is the device this allocator was made for, idle, with every resource bound to its memory destroyed.
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        for memory in self.core.drain() {
            // SAFETY: the caller's promise.
            unsafe { release_memory(device, memory) };
        }
    }

    pub fn stats(&self) -> Stats {
        self.core.stats()
    }
}

/// A device memory given back (unmapped with it).
///
/// # Safety
/// The memory is `device`'s and nothing uses it.
unsafe fn release_memory(device: &ash::Device, memory: DeviceMemory) {
    // SAFETY: the caller's promise; freeing a mapped memory unmaps it.
    unsafe { device.free_memory(memory.memory, None) };
}
