extern crate nbt;
extern crate serde_json;

use std::env;
use std::fs;
use std::io::Read;
use std::process::exit;

use anyhow::anyhow;
use anyhow::Result;

use rayon::prelude::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Biomes {
    palette: Vec<String>,
    data: Option<Vec<i64>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct PaletteBlockProperties {
    level: Option<String>,
    snowy: Option<String>,
    distance: Option<String>,
    persistent: Option<String>,
    waterlogged: Option<String>,
    axis: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct PaletteBlock {
    Properties: Option<PaletteBlockProperties>,
    Name: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct BlockStates {
    palette: Vec<PaletteBlock>,
    data: Option<Vec<i64>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Section {
    Y: i16,
    block_states: BlockStates,
    biomes: Biomes,
    BlockLight: Option<Vec<i8>>,
    SkyLight: Option<Vec<i8>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Chunk {
    DataVersion: i32,
    xPos: i32,
    zPos: i32,
    yPos: i32,
    Status: String,
    LastUpdate: i64,
    sections: Vec<Section>,
}

pub struct RegionFile {
    chunks: [Option<Chunk>; 1024],
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

    let x: nbt::Result<Chunk> = nbt::de::from_zlib_reader(&mut chunk_data);
    x.map_or(Err(anyhow!("Error")), |x| Ok(x))
}

pub fn read_region_file(input: &mut fs::File) -> Result<RegionFile> {
    let data = read_to_vec(input)?;

    let chunks: Vec<_> = (0..1024)
        .into_par_iter()
        .map(|i| -> Option<Chunk> {
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

    let region_file = RegionFile {
        chunks: chunks
            .try_into()
            .unwrap_or_else(|_| panic!("There are not 1024 chunks in the region file")),
    };

    Ok(region_file)
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(arg) = args.into_iter().skip(1).take(1).next() {
        let mut file = fs::File::open(&arg)?;
        let mut region_file = read_region_file(&mut file)?;
        region_file.chunks = region_file.chunks.map(|chunk| {
            chunk.map_blocks_in_place(|blockState| "minecraft:tnt");
            chunk
        });
        file = fs::File::open(&arg)?;
        write_region_file(file);

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
