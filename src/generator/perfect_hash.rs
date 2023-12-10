use std::{num::Wrapping, ops::BitXor};

// hashfunc inspired by FxHasher (rustc-hash)
// unlike FxHasher, this code can't do 64bit ops, because the generated
// perfect hash table should also work if compiled as 32 bit
fn hashfunc(mut data: &[u8]) -> (u32, u32, u32) {
    const HASHCONST1: u32 = 0x541C_69B2; // these 4 constant values are not special, just random values
    const HASHCONST2: u32 = 0x3B17_161B;

    let mut f1 = 0x3314_3C63_u32;
    let mut f2 = 0x88B0_B21E_u32;
    while data.len() >= 4 {
        let val = u32::from_ne_bytes(data[..4].try_into().unwrap());
        f1 = f1.rotate_left(5).bitxor(val).wrapping_mul(HASHCONST1);
        f2 = f2.rotate_left(6).bitxor(val).wrapping_mul(HASHCONST2);
        data = &data[4..];
    }
    if data.len() >= 2 {
        let val = u32::from(u16::from_ne_bytes(data[..2].try_into().unwrap()));
        f1 = f1.rotate_left(5).bitxor(val).wrapping_mul(HASHCONST1);
        f2 = f2.rotate_left(6).bitxor(val).wrapping_mul(HASHCONST2);
        data = &data[2..];
    }
    if !data.is_empty() {
        f1 = f1
            .rotate_left(5)
            .bitxor(u32::from(data[0]))
            .wrapping_mul(HASHCONST1);
        f2 = f2
            .rotate_left(6)
            .bitxor(u32::from(data[0]))
            .wrapping_mul(HASHCONST2);
    }
    let g = f1.bitxor(f2);
    (g, f1, f2)
}

fn displace(f1: u32, f2: u32, d1: u32, d2: u32) -> u32 {
    (Wrapping(d2) + Wrapping(f1) * Wrapping(d1) + Wrapping(f2)).0
}

// this code was copied from rust-phf 0.11 and then modified to better suit the use here
// for a general purpose perfect hash generator see the original code: https://github.com/rust-phf/rust-phf
//
// changes:
// - don't use siphash, instead use the above home-grown hash func, which is inspired by FxHasher (rustc-hash)
// - make lambda a parameter instead of a constant. for some of the input data this allows more compact tables to be generated
pub(crate) fn make_perfect_hash(entries: &[&str], lambda: usize) -> Vec<(u32, u32)> {
    struct Bucket {
        idx: usize,
        keys: Vec<usize>,
    }

    let hashes: Vec<_> = entries
        .iter()
        .map(|entry| hashfunc(entry.as_bytes()))
        .collect();

    let buckets_len = (hashes.len() + lambda - 1) / lambda;
    let mut buckets = (0..buckets_len)
        .map(|i| Bucket {
            idx: i,
            keys: vec![],
        })
        .collect::<Vec<_>>();

    for (i, hash) in hashes.iter().enumerate() {
        buckets[(hash.0 % (buckets_len as u32)) as usize]
            .keys
            .push(i);
    }

    // Sort descending
    buckets.sort_by(|a, b| a.keys.len().cmp(&b.keys.len()).reverse());

    let table_len = hashes.len();
    let table_len_u32 = u32::try_from(table_len).unwrap();
    let mut map = vec![None; table_len];
    let mut disps = vec![(0u32, 0u32); buckets_len];

    // store whether an element from the bucket being placed is
    // located at a certain position, to allow for efficient overlap
    // checks. It works by storing the generation in each cell and
    // each new placement-attempt is a new generation, so you can tell
    // if this is legitimately full by checking that the generations
    // are equal. (A u64 is far too large to overflow in a reasonable
    // time for current hardware.)
    let mut try_map = vec![0u64; table_len];
    let mut generation = 0u64;

    // the actual values corresponding to the markers above, as
    // (index, key) pairs, for adding to the main map once we've
    // chosen the right disps.
    let mut values_to_add = vec![];

    'buckets: for bucket in &buckets {
        for d1 in 0..table_len_u32 {
            'disps: for d2 in 0..table_len_u32 {
                values_to_add.clear();
                generation += 1;

                for &key in &bucket.keys {
                    let idx =
                        (displace(hashes[key].1, hashes[key].2, d1, d2) % table_len_u32) as usize;
                    if map[idx].is_some() || try_map[idx] == generation {
                        continue 'disps;
                    }
                    try_map[idx] = generation;
                    values_to_add.push((idx, key));
                }

                // We've picked a good set of disps
                disps[bucket.idx] = (d1, d2);
                for &(idx, key) in &values_to_add {
                    map[idx] = Some(key);
                }
                continue 'buckets;
            }
        }

        // Unable to find displacements for a bucket
        panic!(
            "perfect hash generation failed ({} items, lambda {lambda})",
            entries.len()
        );
    }

    disps
}

#[inline]
pub(crate) fn get_index(item: &str, disps: &[(u32, u32)], len: usize) -> usize {
    let (g, f1, f2) = hashfunc(item.as_bytes());
    let (d1, d2) = disps[(g % (disps.len() as u32)) as usize];
    (displace(f1, f2, d1, d2) % (len as u32)) as usize
}
