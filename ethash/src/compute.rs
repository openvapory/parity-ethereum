//! Ethash implementation
//! See https://github.com/ethereum/wiki/wiki/Ethash

// TODO: fix endianess for big endian

use std::mem;
use std::ptr;
use sizes::{CACHE_SIZES, DAG_SIZES};
use sha3::{self};

pub const ETHASH_EPOCH_LENGTH: u64 = 30000;
pub const ETHASH_CACHE_ROUNDS: usize = 3;
pub const ETHASH_MIX_BYTES: usize = 128;
pub const ETHASH_ACCESSES:usize =  64;
pub const ETHASH_DATASET_PARENTS:u32 = 256;

const NODE_WORDS: usize = 64 / 4;
const NODE_BYTES: usize = 64;
const MIX_WORDS: usize = ETHASH_MIX_BYTES / 4;
const MIX_NODES: usize = MIX_WORDS / NODE_WORDS;
const FNV_PRIME: u32 =  0x01000193;

/// Computation result
pub struct ProofOfWork {
	/// Difficulty boundary
	pub value: H256,
	/// Mix
	pub mix_hash: H256
}

struct Node {
	bytes: [u8; NODE_BYTES],
}

impl Default for Node {
	fn default() -> Self { 
		Node { bytes: [0u8; NODE_BYTES] }
	}
}

impl Clone for Node {
	fn clone(&self) -> Self {
		Node { bytes: *&self.bytes }
	}
}

impl Node {
	#[inline]
	fn as_words(&self) -> &[u32; NODE_WORDS] {
		unsafe { mem::transmute(&self.bytes) }
	}

	#[inline]
	fn as_words_mut(&mut self) -> &mut [u32; NODE_WORDS] {
		unsafe { mem::transmute(&mut self.bytes) }
	}
}

pub type H256 = [u8; 32];

pub struct Light {
	block_number: u64,
	cache: Vec<Node>,
}

/// Light cache structur
impl Light {
	/// Create a new light cache for a given block number
	pub fn new(block_number: u64) -> Light {
		light_new(block_number)
	}

	/// Calculate the light boundary data
	/// `header_hash` - The header hash to pack into the mix
	/// `nonce` - The nonce to pack into the mix
	pub fn compute(&self, header_hash: &H256, nonce: u64) -> ProofOfWork {
		light_compute(self, header_hash, nonce)
	}
}

#[inline]
fn fnv_hash(x: u32, y: u32) -> u32 {
	return x.wrapping_mul(FNV_PRIME) ^ y;
}

#[inline]
fn sha3_512(input: &[u8], output: &mut [u8]) {
	unsafe { sha3::sha3_512(output.as_mut_ptr(), output.len(), input.as_ptr(), input.len()) };
}

#[inline]
fn get_cache_size(block_number: u64) -> usize {
	assert!(block_number / ETHASH_EPOCH_LENGTH < 2048);
	return CACHE_SIZES[(block_number / ETHASH_EPOCH_LENGTH) as usize] as usize;
}

#[inline]
fn get_data_size(block_number: u64) -> usize {
	assert!(block_number / ETHASH_EPOCH_LENGTH < 2048);
	return DAG_SIZES[(block_number / ETHASH_EPOCH_LENGTH) as usize] as usize;
}

#[inline]
fn get_seedhash(block_number: u64) -> H256 {
	let epochs = block_number / ETHASH_EPOCH_LENGTH;
	let mut ret: H256 = [0u8; 32];
	for _ in 0..epochs {
		unsafe { sha3::sha3_256(ret[..].as_mut_ptr(), 32, ret[..].as_ptr(), 32) };
	}
	ret
}

/// Difficulty quick check for POW preverification
///
/// `header_hash`      The hash of the header
/// `nonce`            The block's nonce
/// `mix_hash`         The mix digest hash
/// Boundary recovered from mix hash
pub fn quick_get_difficulty(header_hash: &H256, nonce: u64, mix_hash: &H256) -> H256 {
	let mut buf = [0u8; 64 + 32];
	unsafe { ptr::copy_nonoverlapping(header_hash.as_ptr(), buf.as_mut_ptr(), 32) };
	unsafe { ptr::copy_nonoverlapping(mem::transmute(&nonce), buf[32..].as_mut_ptr(), 8) };

	unsafe { sha3::sha3_512(buf.as_mut_ptr(), 64, buf.as_ptr(), 40) };
	unsafe { ptr::copy_nonoverlapping(mix_hash.as_ptr(), buf[64..].as_mut_ptr(), 32) };

	let mut hash = [0u8; 32];
	unsafe { sha3::sha3_256(hash.as_mut_ptr(), hash.len(), buf.as_ptr(), buf.len()) };
	hash.as_mut_ptr();
	hash
}

/// Calculate the light client data
/// `light` - The light client handler
/// `header_hash` - The header hash to pack into the mix
/// `nonce` - The nonce to pack into the mix
pub fn light_compute(light: &Light, header_hash: &H256, nonce: u64) -> ProofOfWork {
	let full_size = get_data_size(light.block_number);
	hash_compute(light, full_size, header_hash, nonce)
}

fn hash_compute(light: &Light, full_size: usize,  header_hash: &H256, nonce: u64) -> ProofOfWork {
	if full_size % MIX_WORDS != 0 {
		panic!("Unaligned full size");
	}
	// pack hash and nonce together into first 40 bytes of s_mix
	let mut s_mix: [Node; MIX_NODES + 1] = [ Node::default(), Node::default(), Node::default() ];
	unsafe { ptr::copy_nonoverlapping(header_hash.as_ptr(), s_mix.get_unchecked_mut(0).bytes.as_mut_ptr(), 32) };
	unsafe { ptr::copy_nonoverlapping(mem::transmute(&nonce), s_mix.get_unchecked_mut(0).bytes[32..].as_mut_ptr(), 8) };

	// compute sha3-512 hash and replicate across mix
	unsafe {
		sha3::sha3_512(s_mix.get_unchecked_mut(0).bytes.as_mut_ptr(), NODE_BYTES, s_mix.get_unchecked(0).bytes.as_ptr(), 40);
		let (f_mix, mut mix) = s_mix.split_at_mut(1);
		for w in 0..MIX_WORDS {
			*mix.get_unchecked_mut(0).as_words_mut().get_unchecked_mut(w) = *f_mix.get_unchecked(0).as_words().get_unchecked(w % NODE_WORDS);
		}

		let page_size = 4 * MIX_WORDS;
		let num_full_pages = (full_size / page_size) as u32;

		for i in 0..(ETHASH_ACCESSES as u32) {
			let index = fnv_hash(f_mix.get_unchecked(0).as_words().get_unchecked(0) ^ i, *mix.get_unchecked(0).as_words().get_unchecked((i as usize) % MIX_WORDS)) % num_full_pages;
			for n in 0..MIX_NODES {
				let tmp_node = calculate_dag_item(index * MIX_NODES as u32 + n as u32, light);
				for w in 0..NODE_WORDS {
					*mix.get_unchecked_mut(n).as_words_mut().get_unchecked_mut(w) = fnv_hash(*mix.get_unchecked(n).as_words().get_unchecked(w), *tmp_node.as_words().get_unchecked(w));
				}
			}
		}

		// compress mix
		for i in 0..(MIX_WORDS / 4) {
			let w = i * 4;
			let mut reduction = *mix.get_unchecked(0).as_words().get_unchecked(w + 0);
			reduction = reduction.wrapping_mul(FNV_PRIME) ^ *mix.get_unchecked(0).as_words().get_unchecked(w + 1);
			reduction = reduction.wrapping_mul(FNV_PRIME) ^ *mix.get_unchecked(0).as_words().get_unchecked(w + 2);
			reduction = reduction.wrapping_mul(FNV_PRIME) ^ *mix.get_unchecked(0).as_words().get_unchecked(w + 3);
			*mix.get_unchecked_mut(0).as_words_mut().get_unchecked_mut(i) = reduction;
		}

		let mut mix_hash = [0u8; 32];
		let mut buf = [0u8; 32 + 64];
		ptr::copy_nonoverlapping(f_mix.get_unchecked_mut(0).bytes.as_ptr(), buf.as_mut_ptr(), 64);
		ptr::copy_nonoverlapping(mix.get_unchecked_mut(0).bytes.as_ptr(), buf[64..].as_mut_ptr(), 32);
		ptr::copy_nonoverlapping(mix.get_unchecked_mut(0).bytes.as_ptr(), mix_hash.as_mut_ptr(), 32);
		let mut value: H256 = [0u8; 32];
		sha3::sha3_256(value.as_mut_ptr(), value.len(),  buf.as_ptr(), buf.len());
		ProofOfWork {
			mix_hash: mix_hash,
			value: value,
		}
	}
}

fn calculate_dag_item(node_index: u32, light: &Light) -> Node {
	unsafe {
		let num_parent_nodes = light.cache.len();
		let cache_nodes = &light.cache;
		let init = cache_nodes.get_unchecked(node_index as usize % num_parent_nodes);
		let mut ret = init.clone();
		*ret.as_words_mut().get_unchecked_mut(0) ^= node_index;
		sha3::sha3_512(ret.bytes.as_mut_ptr(), ret.bytes.len(), ret.bytes.as_ptr(), ret.bytes.len());

		for i in 0..ETHASH_DATASET_PARENTS {
			let parent_index = fnv_hash(node_index ^ i, *ret.as_words().get_unchecked(i as usize % NODE_WORDS)) % num_parent_nodes as u32;
			let parent = cache_nodes.get_unchecked(parent_index as usize);
			for w in 0..NODE_WORDS {
				*ret.as_words_mut().get_unchecked_mut(w) = fnv_hash(*ret.as_words().get_unchecked(w), *parent.as_words().get_unchecked(w));
			}
		}
		sha3::sha3_512(ret.bytes.as_mut_ptr(), ret.bytes.len(), ret.bytes.as_ptr(), ret.bytes.len());
		ret
	}
}

fn light_new(block_number: u64) -> Light {
	let seedhash = get_seedhash(block_number);
	let cache_size = get_cache_size(block_number);

	if cache_size % NODE_BYTES != 0 {
		panic!("Unaligned cache size");
	}
	let num_nodes = cache_size / NODE_BYTES;

	let mut nodes = Vec::with_capacity(num_nodes);
	nodes.resize(num_nodes, Node::default());
	unsafe {
		sha3_512(&seedhash[0..32], &mut nodes.get_unchecked_mut(0).bytes);
		for i in 1..num_nodes {
			sha3::sha3_512(nodes.get_unchecked_mut(i).bytes.as_mut_ptr(), NODE_BYTES, nodes.get_unchecked(i - 1).bytes.as_ptr(), NODE_BYTES);
		}
		
		for _ in 0..ETHASH_CACHE_ROUNDS {
			for i in 0..num_nodes {
				let idx = *nodes.get_unchecked_mut(i).as_words().get_unchecked(0) as usize % num_nodes;
				let mut data = nodes.get_unchecked((num_nodes - 1 + i) % num_nodes).clone();
				for w in 0..NODE_WORDS {
					*data.as_words_mut().get_unchecked_mut(w) ^= *nodes.get_unchecked(idx).as_words().get_unchecked(w) ;
				}
				sha3_512(&data.bytes, &mut nodes.get_unchecked_mut(i).bytes);
			}
		}
	}

	Light {
		cache: nodes,
		block_number: block_number,
	}
}

#[test]
fn test_difficulty_test() {
	let hash = [0xf5, 0x7e, 0x6f, 0x3a, 0xcf, 0xc0, 0xdd, 0x4b, 0x5b, 0xf2, 0xbe, 0xe4, 0x0a, 0xb3, 0x35, 0x8a, 0xa6, 0x87, 0x73, 0xa8, 0xd0, 0x9f, 0x5e, 0x59, 0x5e, 0xab, 0x55, 0x94, 0x05,  0x52, 0x7d, 0x72];
	let mix_hash = [0x1f, 0xff, 0x04, 0xce, 0xc9, 0x41, 0x73, 0xfd, 0x59, 0x1e, 0x3d, 0x89, 0x60, 0xce, 0x6b, 0xdf, 0x8b, 0x19, 0x71, 0x04, 0x8c, 0x71, 0xff, 0x93, 0x7b, 0xb2, 0xd3, 0x2a, 0x64, 0x31, 0xab, 0x6d ]; 
	let nonce = 0xd7b3ac70a301a249;
	let boundary_good = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3e, 0x9b, 0x6c, 0x69, 0xbc, 0x2c, 0xe2, 0xa2, 0x4a, 0x8e, 0x95, 0x69, 0xef, 0xc7, 0xd7, 0x1b, 0x33, 0x35, 0xdf, 0x36, 0x8c, 0x9a, 0xe9, 0x7e, 0x53, 0x84];
	assert_eq!(quick_get_difficulty(&hash, nonce, &mix_hash)[..], boundary_good[..]);
	let boundary_bad = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3a, 0x9b, 0x6c, 0x69, 0xbc, 0x2c, 0xe2, 0xa2, 0x4a, 0x8e, 0x95, 0x69, 0xef, 0xc7, 0xd7, 0x1b, 0x33, 0x35, 0xdf, 0x36, 0x8c, 0x9a, 0xe9, 0x7e, 0x53, 0x84];
	assert!(quick_get_difficulty(&hash, nonce, &mix_hash)[..] != boundary_bad[..]);
}

#[test]
fn test_light_compute() {
	let hash = [0xf5, 0x7e, 0x6f, 0x3a, 0xcf, 0xc0, 0xdd, 0x4b, 0x5b, 0xf2, 0xbe, 0xe4, 0x0a, 0xb3, 0x35, 0x8a, 0xa6, 0x87, 0x73, 0xa8, 0xd0, 0x9f, 0x5e, 0x59, 0x5e, 0xab, 0x55, 0x94, 0x05,  0x52, 0x7d, 0x72];
	let mix_hash = [0x1f, 0xff, 0x04, 0xce, 0xc9, 0x41, 0x73, 0xfd, 0x59, 0x1e, 0x3d, 0x89, 0x60, 0xce, 0x6b, 0xdf, 0x8b, 0x19, 0x71, 0x04, 0x8c, 0x71, 0xff, 0x93, 0x7b, 0xb2, 0xd3, 0x2a, 0x64, 0x31, 0xab, 0x6d ]; 
	let boundary = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3e, 0x9b, 0x6c, 0x69, 0xbc, 0x2c, 0xe2, 0xa2, 0x4a, 0x8e, 0x95, 0x69, 0xef, 0xc7, 0xd7, 0x1b, 0x33, 0x35, 0xdf, 0x36, 0x8c, 0x9a, 0xe9, 0x7e, 0x53, 0x84];
	let nonce = 0xd7b3ac70a301a249;
	// difficulty = 0x085657254bd9u64;
	let light = Light::new(486382);
	let result = light_compute(&light, &hash, nonce);
	assert_eq!(result.mix_hash[..], mix_hash[..]);
	assert_eq!(result.value[..], boundary[..]);
}
