//! The allocator's bookkeeping, apart from the API: which memory type a resource goes into, the device memories made
//! (4 MB blocks shared by many resources, or one resource's own), and where in a block each resource is — first fit
//! in an offset-sorted list of free ranges that merge again as they are freed. What it decides the Vulkan side
//! carries out ([`super::GpuAllocator`]); `M` is that side's memory (a `VkDeviceMemory` and where it is mapped).

#![deny(unsafe_code)]

use ash::vk;

/// A block's size, as Skia's: the driver clears what it gives, so a block takes as long as it is big (VMA's 256 MB
/// ones: 128 MB took a frame and more on Adreno, again each time the blocks emptied and filled).
pub const BLOCK_SIZE: u64 = 4 << 20;
/// A resource bigger than this (half a block) has a memory of its own.
pub const DEDICATED_ABOVE: u64 = BLOCK_SIZE / 2;

/// How a resource's memory is laid out: buffers (linear) and optimal-tiling images are kept in blocks of their own,
/// so that `bufferImageGranularity` never applies between neighbours.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tiling {
    Linear,
    Optimal,
}

/// What a resource's memory is for, as memory property flags it must have, should have, and should rather not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MemoryUsage {
    pub required: vk::MemoryPropertyFlags,
    pub preferred: vk::MemoryPropertyFlags,
    pub not_preferred: vk::MemoryPropertyFlags,
}

impl MemoryUsage {
    /// Only the GPU reads and writes it (textures): device-local, not host-visible where there is the choice.
    pub const GPU: MemoryUsage = MemoryUsage {
        required: vk::MemoryPropertyFlags::empty(),
        preferred: vk::MemoryPropertyFlags::DEVICE_LOCAL,
        not_preferred: vk::MemoryPropertyFlags::HOST_VISIBLE,
    };
    /// The CPU writes it once, in order, and the GPU reads it (frame memory): mapped and coherent (spec-guaranteed
    /// to exist), device-local where it can be (resizable BAR, integrated GPUs), not cached.
    pub const UPLOAD: MemoryUsage = MemoryUsage {
        required: vk::MemoryPropertyFlags::from_raw(
            vk::MemoryPropertyFlags::HOST_VISIBLE.as_raw() | vk::MemoryPropertyFlags::HOST_COHERENT.as_raw(),
        ),
        preferred: vk::MemoryPropertyFlags::DEVICE_LOCAL,
        not_preferred: vk::MemoryPropertyFlags::HOST_CACHED,
    };
    /// The GPU writes it and the CPU reads it (readbacks): mapped, cached where it can be.
    pub const READBACK: MemoryUsage = MemoryUsage {
        required: vk::MemoryPropertyFlags::HOST_VISIBLE,
        preferred: vk::MemoryPropertyFlags::from_raw(
            vk::MemoryPropertyFlags::HOST_CACHED.as_raw() | vk::MemoryPropertyFlags::HOST_COHERENT.as_raw(),
        ),
        not_preferred: vk::MemoryPropertyFlags::empty(),
    };
}

/// Flags a type is passed over for unless they are asked for: protected memory, memory only lazily backed, AMD's
/// device-coherent memory (slow, for debugging).
const AVOIDED: vk::MemoryPropertyFlags = vk::MemoryPropertyFlags::from_raw(
    vk::MemoryPropertyFlags::PROTECTED.as_raw()
        | vk::MemoryPropertyFlags::LAZILY_ALLOCATED.as_raw()
        | vk::MemoryPropertyFlags::DEVICE_COHERENT_AMD.as_raw()
        | vk::MemoryPropertyFlags::DEVICE_UNCACHED_AMD.as_raw(),
);

/// The memory types a resource may go into, best first: those `type_bits` (its requirements') allows that have
/// `usage`'s required flags and none avoided, by how many preferred flags they lack plus how many not-preferred ones
/// they have, then by index (the driver lists faster types first).
pub fn memory_type_candidates(
    types: &[vk::MemoryType],
    heaps: &[vk::MemoryHeap],
    type_bits: u32,
    usage: MemoryUsage,
) -> Vec<u32> {
    let mut candidates: Vec<(u32, u32)> = types
        .iter()
        .enumerate()
        .filter(|&(i, t)| {
            i < 32
                && type_bits & (1 << i) != 0
                && t.property_flags.contains(usage.required)
                && (t.property_flags & AVOIDED & !usage.required).is_empty()
                && heaps.get(t.heap_index as usize).is_some_and(|h| h.size > 0)
        })
        .map(|(i, t)| {
            let missing = (usage.preferred & !t.property_flags).as_raw().count_ones();
            let unwanted = (usage.not_preferred & t.property_flags).as_raw().count_ones();
            (missing + unwanted, i as u32)
        })
        .collect();
    candidates.sort();
    candidates.into_iter().map(|(_, i)| i).collect()
}

/// What to allocate: a resource's size and alignment (its memory requirements'), the memory type chosen for it, its
/// tiling, and whether its driver prefers it to have a memory of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Request {
    pub size: u64,
    pub alignment: u64,
    pub memory_type: u32,
    pub tiling: Tiling,
    pub dedicated: bool,
}

/// Where a resource's memory is: which device memory, and where in it. Not `Clone`: freed once.
#[derive(PartialEq, Eq, Debug)]
pub struct Allocation {
    memory: usize,
    offset: u64,
    size: u64,
    memory_type: u32,
}

impl Allocation {
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn memory_type(&self) -> u32 {
        self.memory_type
    }
}

/// What [`AllocatorCore::allocate`] decided.
#[derive(Debug)]
pub enum Step<M> {
    /// In a block there is.
    Placed(Allocation),
    /// A device memory of `size` bytes is to be made (`dedicated`: for the resource alone; else a block) and handed
    /// to [`AllocatorCore::add_memory`], those in `release` given back first (spare blocks, to stay within the
    /// device's count of memories).
    NeedsMemory { size: u64, dedicated: bool, release: Vec<M> },
    /// The device has as many memories as it can, and none to give back.
    OutOfMemories,
}

/// A free range in a block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Range {
    offset: u64,
    size: u64,
}

impl Range {
    fn end(self) -> u64 {
        self.offset + self.size
    }
}

/// A block's free ranges, sorted by offset, none touching another (they merge as they are freed).
#[derive(Clone, PartialEq, Eq, Debug)]
struct FreeList {
    ranges: Vec<Range>,
    used: u64,
}

impl FreeList {
    fn new(size: u64) -> FreeList {
        FreeList { ranges: vec![Range { offset: 0, size }], used: 0 }
    }

    /// The first place `size` bytes fit at a multiple of `alignment`, taken.
    fn allocate(&mut self, size: u64, alignment: u64) -> Option<u64> {
        for i in 0..self.ranges.len() {
            let r = self.ranges[i];
            let start = r.offset.div_ceil(alignment).checked_mul(alignment)?;
            let Some(end) = start.checked_add(size).filter(|&end| end <= r.end()) else { continue };
            let before = Range { offset: r.offset, size: start - r.offset };
            let after = Range { offset: end, size: r.end() - end };
            match (before.size > 0, after.size > 0) {
                (true, true) => {
                    self.ranges[i] = before;
                    self.ranges.insert(i + 1, after);
                }
                (true, false) => self.ranges[i] = before,
                (false, true) => self.ranges[i] = after,
                (false, false) => {
                    self.ranges.remove(i);
                }
            }
            self.used += size;
            return Some(start);
        }
        None
    }

    /// `size` bytes at `offset` free again, merged with the free ranges they touch.
    fn free(&mut self, offset: u64, size: u64) {
        let at = self.ranges.partition_point(|r| r.offset < offset);
        debug_assert!(at == 0 || self.ranges[at - 1].end() <= offset, "freed twice");
        debug_assert!(at == self.ranges.len() || offset + size <= self.ranges[at].offset, "freed twice");
        let joins_before = at > 0 && self.ranges[at - 1].end() == offset;
        let joins_after = at < self.ranges.len() && self.ranges[at].offset == offset + size;
        match (joins_before, joins_after) {
            (true, true) => {
                self.ranges[at - 1].size += size + self.ranges[at].size;
                self.ranges.remove(at);
            }
            (true, false) => self.ranges[at - 1].size += size,
            (false, true) => {
                self.ranges[at].offset = offset;
                self.ranges[at].size += size;
            }
            (false, false) => self.ranges.insert(at, Range { offset, size }),
        }
        self.used -= size;
    }

    fn is_empty(&self) -> bool {
        self.used == 0
    }
}

/// A device memory: a block (with what is free in it) or one resource's own.
struct Memory<M> {
    payload: M,
    memory_type: u32,
    tiling: Tiling,
    size: u64,
    block: Option<FreeList>,
}

impl<M> Memory<M> {
    fn is_spare(&self) -> bool {
        self.block.as_ref().is_some_and(FreeList::is_empty)
    }
}

/// How much is in use, for tests and diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    /// Device memories made and not given back.
    pub memories: usize,
    pub blocks: usize,
    /// Blocks with nothing in them, kept for what comes next.
    pub spare_blocks: usize,
    /// Bytes of device memory.
    pub reserved: u64,
    /// Bytes resources have of it.
    pub used: u64,
}

/// The device memories and what is in them.
pub struct AllocatorCore<M> {
    memories: Vec<Option<Memory<M>>>,
    free_slots: Vec<usize>,
    count: usize,
    max_count: usize,
    block_size: u64,
    dedicated_above: u64,
}

impl<M> AllocatorCore<M> {
    /// For a device that can have `max_count` memories at once (`maxMemoryAllocationCount`), with 4 MB blocks.
    pub fn new(max_count: usize) -> AllocatorCore<M> {
        AllocatorCore::with_block_size(max_count, BLOCK_SIZE)
    }

    /// The same with blocks of `block_size` (resources over half of it have their own).
    pub fn with_block_size(max_count: usize, block_size: u64) -> AllocatorCore<M> {
        AllocatorCore {
            memories: Vec::new(),
            free_slots: Vec::new(),
            count: 0,
            max_count,
            block_size,
            dedicated_above: block_size / 2,
        }
    }

    pub fn block_size(&self) -> u64 {
        self.block_size
    }

    /// Whether a resource this big has a memory of its own.
    pub fn is_dedicated(&self, request: &Request) -> bool {
        request.dedicated || request.size > self.dedicated_above
    }

    /// Where `request` goes: a block there is, or a memory to make.
    pub fn allocate(&mut self, request: &Request) -> Step<M> {
        let size = request.size.max(1);
        let alignment = request.alignment.max(1);
        let dedicated = self.is_dedicated(request);
        if !dedicated {
            for (index, slot) in self.memories.iter_mut().enumerate() {
                let Some(memory) = slot else { continue };
                if memory.memory_type != request.memory_type || memory.tiling != request.tiling {
                    continue;
                }
                let Some(block) = &mut memory.block else { continue };
                if let Some(offset) = block.allocate(size, alignment) {
                    return Step::Placed(Allocation { memory: index, offset, size, memory_type: request.memory_type });
                }
            }
        }
        // A memory to make: room for it made among the device's memories by giving spare blocks back.
        let mut release = Vec::new();
        while self.count >= self.max_count {
            let Some(spare) = self.memories.iter().position(|m| m.as_ref().is_some_and(Memory::is_spare)) else {
                break;
            };
            release.push(self.remove(spare).payload);
        }
        if self.count >= self.max_count {
            return Step::OutOfMemories;
        }
        let size = if dedicated { size } else { self.block_size };
        Step::NeedsMemory { size, dedicated, release }
    }

    /// The memory made as [`Step::NeedsMemory`] asked (a block may be smaller than asked, where the device had no
    /// more, as long as `request` fits), with `request` in it.
    pub fn add_memory(&mut self, request: &Request, size: u64, dedicated: bool, payload: M) -> Allocation {
        let request_size = request.size.max(1);
        let mut block = (!dedicated).then(|| FreeList::new(size));
        let offset = match &mut block {
            Some(block) => {
                block.allocate(request_size, request.alignment.max(1)).expect("the request fits its new block")
            }
            None => 0,
        };
        let memory = Memory { payload, memory_type: request.memory_type, tiling: request.tiling, size, block };
        let index = match self.free_slots.pop() {
            Some(index) => {
                self.memories[index] = Some(memory);
                index
            }
            None => {
                self.memories.push(Some(memory));
                self.memories.len() - 1
            }
        };
        self.count += 1;
        Allocation { memory: index, offset, size: request_size, memory_type: request.memory_type }
    }

    /// The memory an allocation is in.
    pub fn payload(&self, allocation: &Allocation) -> &M {
        &self.memories[allocation.memory].as_ref().expect("an allocation's memory").payload
    }

    /// Whether the allocation has its memory to itself.
    pub fn is_dedicated_allocation(&self, allocation: &Allocation) -> bool {
        self.memories[allocation.memory].as_ref().is_some_and(|m| m.block.is_none())
    }

    /// The allocation's memory's size.
    pub fn memory_size(&self, allocation: &Allocation) -> u64 {
        self.memories[allocation.memory].as_ref().map_or(0, |m| m.size)
    }

    /// The allocation's place free again; the memory to give back, if that leaves it unused: a resource's own, or a
    /// block emptied when its type and tiling have a spare one already (one is kept for what comes next).
    pub fn free(&mut self, allocation: Allocation) -> Option<M> {
        let index = allocation.memory;
        let memory = self.memories[index].as_mut().expect("an allocation's memory");
        let Some(block) = &mut memory.block else { return Some(self.remove(index).payload) };
        block.free(allocation.offset, allocation.size);
        if !block.is_empty() {
            return None;
        }
        let (memory_type, tiling) = (memory.memory_type, memory.tiling);
        let another_spare = self.memories.iter().enumerate().any(|(i, m)| {
            i != index && m.as_ref().is_some_and(|m| m.memory_type == memory_type && m.tiling == tiling && m.is_spare())
        });
        another_spare.then(|| self.remove(index).payload)
    }

    fn remove(&mut self, index: usize) -> Memory<M> {
        let memory = self.memories[index].take().expect("a memory");
        self.free_slots.push(index);
        self.count -= 1;
        memory
    }

    /// Every memory, given back (at the end).
    pub fn drain(&mut self) -> Vec<M> {
        self.free_slots.clear();
        self.count = 0;
        self.memories.drain(..).flatten().map(|m| m.payload).collect()
    }

    pub fn stats(&self) -> Stats {
        let mut stats = Stats::default();
        for m in self.memories.iter().flatten() {
            stats.memories += 1;
            stats.reserved += m.size;
            match &m.block {
                Some(block) => {
                    stats.blocks += 1;
                    stats.used += block.used;
                    stats.spare_blocks += block.is_empty() as usize;
                }
                None => stats.used += m.size,
            }
        }
        stats
    }

    /// Checks what it keeps adds up (tests): every block's free ranges sorted, apart, within it, and with what is
    /// used its size.
    pub fn check(&self) {
        let mut count = 0;
        for m in self.memories.iter().flatten() {
            count += 1;
            let Some(block) = &m.block else { continue };
            let mut free = 0;
            for pair in block.ranges.windows(2) {
                assert!(pair[0].end() < pair[1].offset, "free ranges touching or overlapping: {pair:?}");
            }
            for r in &block.ranges {
                assert!(r.size > 0 && r.end() <= m.size, "a free range out of its block: {r:?}");
                free += r.size;
            }
            assert_eq!(free + block.used, m.size, "a block's free and used bytes");
        }
        assert_eq!(count, self.count);
        assert!(self.count <= self.max_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KB: u64 = 1024;

    fn request(size: u64, alignment: u64) -> Request {
        Request { size, alignment, memory_type: 0, tiling: Tiling::Optimal, dedicated: false }
    }

    /// Allocates as the Vulkan side does: placed, or a new memory (a counter for its payload).
    fn allocate(core: &mut AllocatorCore<u32>, next: &mut u32, request: &Request) -> Option<Allocation> {
        match core.allocate(request) {
            Step::Placed(a) => Some(a),
            Step::NeedsMemory { size, dedicated, .. } => {
                *next += 1;
                Some(core.add_memory(request, size, dedicated, *next))
            }
            Step::OutOfMemories => None,
        }
    }

    fn types(flags: &[vk::MemoryPropertyFlags]) -> (Vec<vk::MemoryType>, Vec<vk::MemoryHeap>) {
        let types = flags.iter().map(|&f| vk::MemoryType { property_flags: f, heap_index: 0 }).collect();
        (types, vec![vk::MemoryHeap { size: 1 << 30, flags: vk::MemoryHeapFlags::DEVICE_LOCAL }])
    }

    #[test]
    fn resources_go_where_they_are_best() {
        use vk::MemoryPropertyFlags as F;
        // A discrete GPU's: device-local; host memory, coherent and cached; resizable BAR.
        let (t, h) = types(&[
            F::DEVICE_LOCAL,
            F::HOST_VISIBLE | F::HOST_COHERENT,
            F::HOST_VISIBLE | F::HOST_COHERENT | F::HOST_CACHED,
            F::DEVICE_LOCAL | F::HOST_VISIBLE | F::HOST_COHERENT,
        ]);
        // (Device-local first, host-visible last where it can be.)
        assert_eq!(memory_type_candidates(&t, &h, !0, MemoryUsage::GPU), [0, 3, 1, 2]);
        assert_eq!(memory_type_candidates(&t, &h, !0, MemoryUsage::UPLOAD), [3, 1, 2]);
        assert_eq!(memory_type_candidates(&t, &h, !0, MemoryUsage::READBACK), [2, 1, 3]);
        // Only what the resource allows.
        assert_eq!(memory_type_candidates(&t, &h, 0b0110, MemoryUsage::GPU), [1, 2]);
        assert_eq!(memory_type_candidates(&t, &h, 0b0001, MemoryUsage::UPLOAD), Vec::<u32>::new());
        // A CPU's (lavapipe): everything everywhere, device-local first.
        let (t, h) = types(&[F::DEVICE_LOCAL | F::HOST_VISIBLE | F::HOST_COHERENT | F::HOST_CACHED]);
        for usage in [MemoryUsage::GPU, MemoryUsage::UPLOAD, MemoryUsage::READBACK] {
            assert_eq!(memory_type_candidates(&t, &h, !0, usage), [0]);
        }
        // Protected and lazily allocated memory passed over; a heap of nothing too.
        let (t, mut h) =
            types(&[F::DEVICE_LOCAL | F::PROTECTED, F::DEVICE_LOCAL | F::LAZILY_ALLOCATED, F::HOST_VISIBLE]);
        assert_eq!(memory_type_candidates(&t, &h, !0, MemoryUsage::GPU), [2]);
        h[0].size = 0;
        assert_eq!(memory_type_candidates(&t, &h, !0, MemoryUsage::GPU), Vec::<u32>::new());
    }

    #[test]
    fn small_resources_share_a_block() {
        let mut core = AllocatorCore::new(4096);
        let mut next = 0;
        let a = allocate(&mut core, &mut next, &request(1000, 256)).unwrap();
        let b = allocate(&mut core, &mut next, &request(1000, 256)).unwrap();
        let c = allocate(&mut core, &mut next, &request(64 * KB, 64 * KB)).unwrap();
        assert_eq!((a.offset(), b.offset(), c.offset()), (0, 1024, 64 * KB));
        assert_eq!(core.payload(&a), core.payload(&c));
        assert_eq!(core.memory_size(&a), BLOCK_SIZE);
        assert_eq!(
            core.stats(),
            Stats { memories: 1, blocks: 1, spare_blocks: 0, reserved: BLOCK_SIZE, used: 1000 + 1000 + 64 * KB }
        );
        // The gap before c is used by what fits it (at a multiple of 16).
        let d = allocate(&mut core, &mut next, &request(2000, 16)).unwrap();
        assert_eq!(d.offset(), 2032);
        core.check();
    }

    #[test]
    fn freed_ranges_merge_again() {
        let mut core = AllocatorCore::new(4096);
        let mut next = 0;
        let all: Vec<Allocation> =
            (0..8).map(|_| allocate(&mut core, &mut next, &request(4 * KB, 4 * KB)).unwrap()).collect();
        let offsets: Vec<u64> = all.iter().map(Allocation::offset).collect();
        assert_eq!(offsets, (0..8).map(|i| i * 4 * KB).collect::<Vec<_>>());
        // Every other one freed, then the rest: one range again, the whole block.
        let (even, odd): (Vec<_>, Vec<_>) = all.into_iter().enumerate().partition(|(i, _)| i % 2 == 0);
        for (_, a) in even {
            assert_eq!(core.free(a), None);
            core.check();
        }
        let big = allocate(&mut core, &mut next, &request(8 * KB, 4 * KB)).unwrap();
        assert_eq!(big.offset(), 32 * KB, "no two neighbouring gaps yet");
        for (_, a) in odd {
            assert_eq!(core.free(a), None, "the last block of its kind is kept");
            core.check();
        }
        assert_eq!(core.free(big), None);
        core.check();
        let s = core.stats();
        assert_eq!((s.blocks, s.spare_blocks, s.used), (1, 1, 0));
        // Whole again: half a block fits at its start, in the block there is.
        let half = allocate(&mut core, &mut next, &request(BLOCK_SIZE / 2, 4 * KB)).unwrap();
        assert_eq!((half.offset(), next), (0, 1));
    }

    #[test]
    fn big_resources_have_memories_of_their_own() {
        let mut core = AllocatorCore::new(4096);
        let mut next = 0;
        let big = allocate(&mut core, &mut next, &request(DEDICATED_ABOVE + 1, 256)).unwrap();
        assert!(core.is_dedicated_allocation(&big));
        assert_eq!(core.memory_size(&big), DEDICATED_ABOVE + 1);
        let half = allocate(&mut core, &mut next, &request(DEDICATED_ABOVE, 256)).unwrap();
        assert!(!core.is_dedicated_allocation(&half));
        let preferred = allocate(&mut core, &mut next, &Request { dedicated: true, ..request(4 * KB, 256) }).unwrap();
        assert!(core.is_dedicated_allocation(&preferred));
        assert_eq!(core.stats().memories, 3);
        // Given back as soon as freed.
        assert_eq!(core.free(big), Some(1));
        assert_eq!(core.free(preferred), Some(3));
        assert_eq!(core.free(half), None);
        core.check();
    }

    #[test]
    fn kinds_and_types_keep_to_their_blocks() {
        let mut core = AllocatorCore::new(4096);
        let mut next = 0;
        let image = allocate(&mut core, &mut next, &request(1000, 16)).unwrap();
        let buffer = allocate(&mut core, &mut next, &Request { tiling: Tiling::Linear, ..request(1000, 16) }).unwrap();
        let other = allocate(&mut core, &mut next, &Request { memory_type: 3, ..request(1000, 16) }).unwrap();
        assert_eq!((core.payload(&image), core.payload(&buffer), core.payload(&other)), (&1, &2, &3));
        assert_eq!(other.memory_type(), 3);
        assert!([&image, &buffer, &other].iter().all(|a| a.offset() == 0));
    }

    #[test]
    fn one_spare_block_is_kept() {
        let mut core = AllocatorCore::new(4096);
        let mut next = 0;
        // Three blocks' worth.
        let all: Vec<Allocation> =
            (0..6).map(|_| allocate(&mut core, &mut next, &request(2 * 1024 * KB, 4))).map(Option::unwrap).collect();
        assert_eq!(core.stats().blocks, 3);
        let mut released = Vec::new();
        for a in all {
            released.extend(core.free(a));
            core.check();
        }
        assert_eq!(released.len(), 2);
        let s = core.stats();
        assert_eq!((s.blocks, s.spare_blocks, s.reserved), (1, 1, BLOCK_SIZE));
    }

    #[test]
    fn the_count_of_memories_is_kept_to() {
        let mut core = AllocatorCore::new(2);
        let mut next = 0;
        let a = allocate(&mut core, &mut next, &Request { memory_type: 0, ..request(16, 16) }).unwrap();
        let b = allocate(&mut core, &mut next, &Request { memory_type: 1, ..request(16, 16) }).unwrap();
        assert!(matches!(core.allocate(&Request { memory_type: 2, ..request(16, 16) }), Step::OutOfMemories));
        // An emptied block (the spare of its type) is given back to make room for another type's.
        assert_eq!(core.free(a), None);
        match core.allocate(&Request { memory_type: 2, ..request(16, 16) }) {
            Step::NeedsMemory { size, dedicated, release } => {
                assert_eq!((size, dedicated, release), (BLOCK_SIZE, false, vec![1]));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(core.stats().memories, 1);
        assert_eq!(core.payload(&b), &2);
    }

    #[test]
    fn smaller_blocks_do_when_the_device_has_no_more() {
        let mut core: AllocatorCore<u32> = AllocatorCore::new(4096);
        let r = request(300 * KB, 64);
        let Step::NeedsMemory { size, .. } = core.allocate(&r) else { panic!() };
        // Half of it made instead.
        let a = core.add_memory(&r, size / 2, false, 7);
        assert_eq!(core.memory_size(&a), BLOCK_SIZE / 2);
        let b = allocate(&mut core, &mut 7, &r).unwrap();
        assert_eq!((b.offset(), core.payload(&b)), (300 * KB, &7));
        core.check();
    }

    #[test]
    fn a_long_run_adds_up() {
        // Sizes and alignments as textures and buffers have them, allocated and freed in a shuffled order.
        let mut core = AllocatorCore::with_block_size(64, 256 * KB);
        let mut next = 0;
        let mut live: Vec<Allocation> = Vec::new();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut random = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let steps = if cfg!(miri) { 300 } else { 5000 };
        for _ in 0..steps {
            if live.len() < 150 && (live.is_empty() || random(2) == 0) {
                let alignment = 1 << random(14);
                let largest = if random(10) == 0 { 300 * KB } else { 40 * KB };
                let size = 1 + random(largest);
                let tiling = if random(2) == 0 { Tiling::Linear } else { Tiling::Optimal };
                let r = Request { size, alignment, memory_type: random(2) as u32, tiling, dedicated: random(50) == 0 };
                let a = allocate(&mut core, &mut next, &r).expect("room");
                assert_eq!(a.offset() % alignment, 0);
                // Apart from every other allocation in its memory.
                for b in &live {
                    if core.payload(b) == core.payload(&a) {
                        assert!(
                            a.offset() + a.size() <= b.offset() || b.offset() + b.size() <= a.offset(),
                            "{a:?} overlaps {b:?}"
                        );
                    }
                }
                live.push(a);
            } else {
                let a = live.swap_remove(random(live.len() as u64) as usize);
                core.free(a);
            }
            core.check();
        }
        for a in live {
            core.free(a);
        }
        core.check();
        let s = core.stats();
        assert_eq!(s.used, 0);
        assert!(s.memories <= 4, "at most a spare block per type and tiling: {s:?}");
        assert_eq!(core.drain().len(), s.memories);
        assert_eq!(core.stats(), Stats::default());
    }
}
