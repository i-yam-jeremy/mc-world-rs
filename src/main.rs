extern crate nbt;
extern crate serde_json;

use std::env;
use std::fs;
use std::io::Read;
use std::process::exit;

use nbt::Result;
use rayon::prelude::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
pub struct Chunk {
    DataVersion: i32,
    xPos: i32,
    zPos: i32,
    yPos: i32,
}

struct ChunkOffset {
    pub offset: u32, // Offset in file in 4KiB blocks from the start of the region file
    pub sector_count: u32,
}

fn read_to_vec(input: &mut fs::File) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(input.metadata()?.len().try_into().unwrap());
    input.read_to_end(&mut data)?;
    Ok(data)
}

fn get_u32(data: &[u8], byte_offset: usize) -> u32 {
    ((data[byte_offset + 3] as u32) << 24)
        + ((data[byte_offset + 2] as u32) << 16)
        + ((data[byte_offset + 1] as u32) << 8)
        + ((data[byte_offset + 0] as u32) << 0)
}

fn read_chunk_offset(data: &[u8], byte_offset: usize) -> ChunkOffset {
    ChunkOffset {
        offset: ((data[byte_offset + 0] as u32) << 16)
            + ((data[byte_offset + 1] as u32) << 8)
            + ((data[byte_offset + 2] as u32) << 0),
        sector_count: data[byte_offset + 3] as u32,
    }
}

fn read_chunk(data: &[u8], offset: &ChunkOffset) -> Result<Chunk> {
    let base_offset = (offset.offset as usize) * 4096;
    // let length = get_u32(data, base_offset) as usize;
    let compression = data[base_offset + 4];
    if compression != 2 {
        panic!("Unsupported chunk compression type: {}", compression);
    }
    let mut chunk_data = &data[base_offset + 5..];

    Ok(nbt::de::from_zlib_reader(&mut chunk_data)?)
}

pub fn read_region_file(input: &mut fs::File) -> Result<Vec<Chunk>> {
    let data = read_to_vec(input)?;

    let chunks: Vec<_> = (0..1024)
        .into_par_iter()
        .filter_map(|i| -> Option<Chunk> {
            // let x = i & 31;
            // let z = (i >> 5) & 31;
            let offset = read_chunk_offset(&data[..], 4 * i);
            let timestamp = get_u32(&data[..], 4 * i + 4096);
            if offset.offset == 0 && offset.sector_count == 0 && timestamp == 0 {
                // Unloaded chunk
                return None;
            }
            let chunk = read_chunk(&data, &offset);
            chunk.map_or(None, |chunk| Some(chunk))
        })
        .collect();

    Ok(chunks)
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(arg) = args.into_iter().skip(1).take(1).next() {
        let mut file = fs::File::open(&arg)?;
        let chunks = read_region_file(&mut file)?;
        for chunk in chunks {
            println!("{:?}", chunk);
        }
        Ok(())
    } else {
        eprintln!("error: a filename is required.");
        exit(1)
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {}", err);
        exit(1)
    };
}
