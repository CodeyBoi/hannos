use core::{mem::size_of, num::NonZeroU32};

use alloc::vec::Vec;

use super::disk;

// As the block at index 0 is the superblock which should rarely be referenced, we can assert that a block pointer
// is non-zero. This will also enable Rust's `null pointer optimization` which will make `Option<BlockPtr>` take
// up less space (as it can use a value of zero as the value for `None`)
type BlockPtr = NonZeroU32;
type INumber = u32;

const MAGIC_NUMBER: usize = 0xdeadbeef;
const INODES_PER_BLOCK: usize = disk::BLOCK_SIZE / size_of::<Inode>();
const PTRS_PER_INODE: usize = 11;
const PTRS_PER_BLOCK: usize = disk::BLOCK_SIZE / size_of::<Option<BlockPtr>>();
const INODE_BLOCKS_START: usize = 1;

#[derive(Clone)]
pub struct Inode {
    valid: bool,
    size: usize,
    direct: [Option<BlockPtr>; 11],
    indirect: Option<BlockPtr>,
}

pub struct FileSystem {
    superblock: Superblock,
    block_bitmap: Vec<u64>,
}

struct Superblock {
    magic_number: usize,
    blocks: usize,
    inode_blocks: usize,
    inodes: usize,
}
type InodeBlock = [Inode; INODES_PER_BLOCK];
type PointerBlock = [Option<BlockPtr>; PTRS_PER_BLOCK];
type DataBlock = [u8; disk::BLOCK_SIZE];

union Block<'a> {
    superblock: &'a Superblock,
    inodes: &'a InodeBlock,
    pointers: &'a PointerBlock,
    data: &'a DataBlock,
}

impl<'a> Block<'a> {
    fn from_le_bytes(bytes: &'a [u8]) -> Option<&'a Block> {
        let ptr = bytes as *const _ as *const Block;
        unsafe { ptr.as_ref() }
    }
}

impl Inode {
    fn new(valid: bool) -> Self {
        Self {
            valid,
            size: 0,
            direct: [None; PTRS_PER_INODE],
            indirect: None,
        }
    }
}

impl FileSystem {
    pub fn new() -> Self {
        Self {
            superblock: Superblock {
                magic_number: 0,
                blocks: 0,
                inode_blocks: 0,
                inodes: 0,
            },
            block_bitmap: Vec::new(),
        }
    }

    pub fn format() {
        // The superblock should be formatted as [MAGIC_NUMBER, BLOCKS, INODE_BLOCKS, INODES]
        let blocks = disk::size();
        let inode_blocks = blocks / 10 + 1;
        let inodes = inode_blocks * INODES_PER_BLOCK;
        let superblock: Vec<u8> = [MAGIC_NUMBER, blocks, inode_blocks, inodes]
            .iter()
            .map(|v| v.to_le_bytes())
            .flatten()
            .collect();

        // Write the superblock to disk block 0 (the first block)
        disk::write(0, 0, &superblock).unwrap();

        // Clear all inode blocks
        let zero_data = [0u8; disk::BLOCK_SIZE];
        for i in INODE_BLOCKS_START..inode_blocks + INODE_BLOCKS_START {
            disk::write(i, 0, &zero_data).unwrap();
        }
    }

    pub fn mount(&mut self) {
        let mut buf = [0; disk::BLOCK_SIZE];

        let sb = Self::read_block(0, &mut buf);
        let sb = unsafe { sb.superblock };

        if sb.magic_number != MAGIC_NUMBER {
            panic!("encountered invalid magic number while mounting drive");
        }

        self.block_bitmap = (0..sb.blocks / u64::BITS as usize)
            .map(|_| 0xbbbbbbbbbbbbbbbb) // 0b1111...
            .collect();

        // Mark the first block (index 0) as used, as it's the superblock
        self.block_bitmap[0] &= !(1);

        for block_idx in INODE_BLOCKS_START..sb.inode_blocks + INODE_BLOCKS_START {
            let block = Self::read_block(block_idx, &mut buf);

            // Mark inode blocks as used
            self.mark_block(BlockPtr::new(block_idx as u32).unwrap(), false);

            for inode in unsafe { block.inodes } {
                if !inode.valid {
                    continue;
                }

                for block in inode.direct {
                    if let Some(block) = block {
                        self.mark_block(block, false);
                    }
                }
            }
        }
    }

    pub fn create(&self) -> Option<INumber> {
        let inumber = self.next_free_inode()?;
        let file = Inode::new(true);
        Self::write_inode(inumber, &file);
        Some(inumber)
    }

    pub fn delete(&mut self, inumber: INumber) {
        // Mark all directly pointed to data blocks as free
        let inode = Self::read_inode(inumber);
        for ptr in inode.direct {
            if let Some(block_idx) = ptr {
                self.mark_block(block_idx, true);
            }
        }

        // Mark all indirectly pointed to data blocks as free
        if let Some(block) = inode.indirect {
            let mut buf = [0; disk::BLOCK_SIZE];
            let block = Self::read_block(block.get() as usize, &mut buf);
            for ptr in unsafe { block.pointers } {
                if let &Some(block_idx) = ptr {
                    self.mark_block(block_idx, true)
                }
            }
        }

        // Overwrite the inode
        let new_inode = Inode::new(false);
        Self::write_inode(inumber, &new_inode);
    }

    pub fn read(&self, inumber: INumber, offset: usize, outbuf: &mut [u8]) -> Result<usize, ()> {
        let inode = Self::read_inode(inumber);

        if inode.size <= offset {
            return Err(());
        }

        let bytes_to_read = outbuf.len().min(inode.size - offset);
        let mut bytes_read = 0;

        let (first_ptr_idx, first_offset) = (offset / disk::BLOCK_SIZE, offset % disk::BLOCK_SIZE);
        if first_ptr_idx < inode.direct.len() {
            // Read the first block. This is the only block that could need an offset from the start
            bytes_read += Self::read_raw_data_many(
                &inode.direct[first_ptr_idx..],
                first_offset,
                bytes_to_read - bytes_read,
                outbuf,
            );

            // If we are done reading, return
            if bytes_read >= bytes_to_read {
                return Ok(bytes_read);
            }

            // Otherwise, keep reading from the indirect pointers
            if let Some(ptr) = inode.indirect {
                let mut buf = [0; disk::BLOCK_SIZE];
                let pointers = Self::read_pointer_block(ptr, &mut buf);
                bytes_read +=
                    Self::read_raw_data_many(pointers, 0, bytes_to_read - bytes_read, outbuf);
            }
        } else if let Some(ptr) = inode.indirect {
            // Offset puts us into the indirect pointers from the start
            let mut buf = [0; disk::BLOCK_SIZE];
            let pointers = Self::read_pointer_block(ptr, &mut buf);
            bytes_read += Self::read_raw_data_many(
                &pointers[first_ptr_idx - inode.direct.len()..],
                first_offset,
                bytes_to_read - bytes_read,
                outbuf,
            );
        }

        Ok(bytes_read)
    }

    pub fn write(&mut self, inumber: INumber, offset: usize, data: &[u8]) -> Result<usize, ()> {
        let inode = Self::read_inode(inumber);
        let allocated_blocks = Self::allocated_blocks(inode.size);
        let new_size = offset + data.len();
        let new_allocated_blocks = Self::allocated_blocks(new_size);

        // Allocate or deallocate blocks if needed

        Ok(0)
    }

    fn allocated_blocks(size: usize) -> usize {
        if size == 0 {
            0
        } else {
            (size - 1) / disk::BLOCK_SIZE + 1
        }
    }

    /// Marks a block as free or busy. Values of zero for the block index are disallowed, as that's the index of the superblock.
    fn mark_block(&mut self, block: BlockPtr, free: bool) {
        let block_idx = block.get();
        let (idx, offset) = ((block_idx / u64::BITS) as usize, block_idx % u64::BITS);
        self.block_bitmap[idx] &= !(1 << offset);
        self.block_bitmap[idx] |= 1 << free as u64;
    }

    fn is_free(&self, block: BlockPtr) -> bool {
        let block_idx = block.get();
        let (idx, offset) = ((block_idx / u64::BITS) as usize, block_idx % u64::BITS);
        self.block_bitmap[idx] & (1 << offset) > 0
    }

    /// Finds the next free inode and returns its `inumber`.
    fn next_free_inode(&self) -> Option<INumber> {
        let mut buf = [0; disk::BLOCK_SIZE];
        for block_idx in INODE_BLOCKS_START..self.superblock.inode_blocks + INODE_BLOCKS_START {
            let block = Self::read_block(block_idx, &mut buf);
            for (offset, inode) in unsafe { block.inodes }.iter().enumerate() {
                let file = unsafe { (inode as *const _ as *const Inode).as_ref().unwrap() };
                if !file.valid {
                    let inumber = (block_idx - INODE_BLOCKS_START) * INODES_PER_BLOCK + offset;
                    return Some(inumber as INumber);
                }
            }
        }
        None
    }

    fn next_free_block(&self) -> Option<BlockPtr> {
        let (idx, &bitmask) = self
            .block_bitmap
            .iter()
            .enumerate()
            .find(|(_, &value)| value > 0)?;
        let first_one_idx = bitmask.trailing_zeros(); // number of trailing 0 will give the index of the first 1
        NonZeroU32::new(idx as u32 * u64::BITS + first_one_idx)
    }

    fn read_raw_data(block: BlockPtr, offset: usize, length: usize, outbuf: &mut [u8]) -> usize {
        let mut buf = [0; disk::BLOCK_SIZE];
        let block = Self::read_block(block.get() as usize, &mut buf);
        let block_data = unsafe { block.data };
        let block_data = &block_data[offset..block_data.len().min(offset + length)];

        let mut bytes_read = 0;
        for (out, data) in outbuf.iter_mut().zip(block_data) {
            *out = *data;
            bytes_read += 1;
        }
        bytes_read
    }

    fn read_raw_data_many(
        blocks: &[Option<BlockPtr>],
        mut offset: usize,
        length: usize,
        outbuf: &mut [u8],
    ) -> usize {
        let mut bytes_read = 0;
        let bytes_to_read = outbuf.len().min(length);

        for ptr in blocks {
            if let &Some(block_ptr) = ptr {
                bytes_read +=
                    Self::read_raw_data(block_ptr, offset, bytes_to_read - bytes_read, outbuf);
                offset = 0; // set offset to 0 as we only want the offset for the first block
            } else {
                panic!("null block pointer in inode");
            }

            if bytes_read >= bytes_to_read {
                break;
            }
        }

        bytes_read
    }

    fn read_block<'a>(block: usize, outbuf: &'a mut [u8]) -> &'a Block {
        disk::read(block, 0, outbuf).expect("error when reading storage block");
        Block::from_le_bytes(outbuf).expect("error when casting raw data as storage block")
    }

    fn read_pointer_block<'a>(block: BlockPtr, outbuf: &'a mut [u8]) -> &'a PointerBlock {
        let block = Self::read_block(block.get() as usize, outbuf);
        unsafe { block.pointers }
    }

    fn write_inode(inumber: INumber, file: &Inode) {
        let (block, offset) = Self::calc_inode_pos(inumber);
        let buf_ptr = file as *const _ as *const [u8; size_of::<Inode>()];
        disk::write(block, offset, unsafe { buf_ptr.as_ref().unwrap() }).unwrap();
    }

    fn read_inode(inumber: INumber) -> Inode {
        let (block, offset) = Self::calc_inode_pos(inumber);
        let mut buf = [0; size_of::<Inode>()];
        disk::read(block, offset, &mut buf).unwrap();
        let file_ptr = &buf as *const _ as *const Inode;
        unsafe { file_ptr.as_ref() }.unwrap().clone()
    }

    fn calc_inode_pos(inumber: INumber) -> (usize, usize) {
        (
            inumber as usize / INODES_PER_BLOCK + INODE_BLOCKS_START,
            inumber as usize % INODES_PER_BLOCK,
        )
    }
}
