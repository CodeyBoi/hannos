use core::fmt::Debug;

use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use thiserror_no_std::Error;

pub const BLOCK_SIZE: usize = 0x1000;

lazy_static! {
    static ref DISK: Mutex<Disk> = Mutex::new(Disk::new(1024));
}

/// Read a block from the disk into a buffer.
///
/// # Panics
/// If the offset and length of the buffer exceed the block size.
pub fn read(block: usize, offset: usize, buf: &mut [u8]) -> Result<(), DiskError> {
    DISK.lock().read(block, offset, buf)
}

/// Write a buffer to a block on the disk.
///
/// # Panics
/// If the offset and length of the buffer exceed the block size.
pub fn write(block: usize, offset: usize, buf: &[u8]) -> Result<(), DiskError> {
    DISK.lock().write(block, offset, buf)
}

/// Returns the size of the disk in blocks.
pub fn size() -> usize {
    DISK.lock().size()
}

struct DiskBlock {
    data: [u8; BLOCK_SIZE],
}

struct Disk {
    blocks: Vec<DiskBlock>,
}

pub trait BlockDevice {
    fn read(&self, block: usize, buf: &mut [u8]);
    fn write(&mut self, block: usize, buf: &[u8]);
}

#[derive(Error, Debug)]
pub enum DiskError {
    #[error("block {0} out of bounds")]
    BlockOutOfBounds(usize),
    #[error("tried to write {0} bytes at offset {1}, which exceeds block size of {BLOCK_SIZE}")]
    BufferTooLarge(usize, usize),
}

impl Disk {
    /// Creates an simulated disk with the given number of blocks.
    /// Each block is 4 KiB.
    fn new(blocks: usize) -> Self {
        Self {
            blocks: (0..blocks)
                .map(|_| DiskBlock {
                    data: [0; BLOCK_SIZE],
                })
                .collect(),
        }
    }

    fn size(&self) -> usize {
        self.blocks.len()
    }

    fn read(&self, block: usize, offset: usize, buf: &mut [u8]) -> Result<(), DiskError> {
        if block >= self.blocks.len() {
            return Err(DiskError::BlockOutOfBounds(block));
        }

        let block = &self.blocks[block];
        buf.copy_from_slice(&block.data[offset..offset + buf.len()]);
        Ok(())
    }

    fn write(&mut self, block: usize, offset: usize, buf: &[u8]) -> Result<(), DiskError> {
        if block >= self.blocks.len() {
            return Err(DiskError::BlockOutOfBounds(block));
        }

        if offset + buf.len() > BLOCK_SIZE {
            return Err(DiskError::BufferTooLarge(buf.len(), offset));
        }

        let block = &mut self.blocks[block];
        block.data[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(())
    }
}

impl BlockDevice for Disk {
    fn read(&self, block: usize, buf: &mut [u8]) {
        self.read(block, 0, buf).unwrap();
    }

    fn write(&mut self, block: usize, buf: &[u8]) {
        self.write(block, 0, buf).unwrap();
    }
}
