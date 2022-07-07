#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

pub(crate) fn find_hash_parameters(input_strings: &[&str]) -> Option<(usize, usize, usize)> {
    let upper_limit = input_strings.len();
    let lower_limit = upper_limit / 2;
    for hashlen in lower_limit..upper_limit {
        println!("testing hashlen {hashlen}");
        let now = std::time::Instant::now();
        let distvalues = find_best_distribution(input_strings, hashlen);
        for idx1 in 0..(distvalues.len() - 1) {
            let param1 = distvalues[idx1];
            for param2 in distvalues.iter().skip(idx1 + 1) {
                if let Ok((_table1, _table2)) =
                    make_perfect_hash(input_strings, param1, *param2, hashlen)
                {
                    println!(
                        "hash func generated @hashlen={hashlen}, param1={param1}, param2={param2}"
                    );
                    println!(
                        "size factor = {}",
                        hashlen as f64 / input_strings.len() as f64
                    );
                    return Some((hashlen, param1, *param2));
                }
            }
        }
        // let param1 =
        // println!("   param1 = {param1}");
        // for param2 in 257..5102 {

        // }
        println!(
            "   no success after {}s",
            now.elapsed().as_millis() as f64 / 1000.0
        );
    }

    None
}

fn find_best_distribution(input_strings: &[&str], hashsize: usize) -> Vec<usize> {
    let mut distributions = Vec::with_capacity(input_strings.len());
    // limits: lower limit 257 -> this ensures the multiplication step wil lalways create a value that cannot be gotten just through adding an u8
    //         upper limit 65535 -> the higher bits don't contribute much to the end result, so trying higher values is likely a waste of time
    for param in 257..65538 {
        let mut buckets = vec![0; hashsize];
        for in_str in input_strings {
            let hashval = hashfunc(in_str.as_bytes(), param) % hashsize;
            buckets[hashval] += 1;
        }

        let distval = buckets.iter().fold(0, |acc, val| acc + (val * val)) - input_strings.len();
        distributions.push((distval, param));
    }

    distributions.sort_by(|item1, item2| item1.0.cmp(&item2.0));
    let mut outvalues: Vec<usize> = distributions.iter().map(|item| item.1).collect();
    outvalues.truncate(200);
    outvalues
}

pub(crate) fn make_perfect_hash(
    input_strings: &[&str],
    param1: usize,
    param2: usize,
    hashlen: usize,
) -> Result<(Vec<u16>, Vec<u16>), String> {
    let datalen = input_strings.len();
    let mut buckets1 = HashMap::<usize, HashSet<&str>>::with_capacity(hashlen);
    let mut buckets2 = HashMap::<usize, HashSet<&str>>::with_capacity(hashlen);

    for i in 0..hashlen {
        buckets1.insert(i, HashSet::new());
        buckets2.insert(i, HashSet::new());
    }

    for in_str in input_strings {
        let hashval1 = hashfunc(in_str.as_bytes(), param1) % hashlen;
        let hashval2 = hashfunc(in_str.as_bytes(), param2) % hashlen;
        let set1 = buckets1.get_mut(&hashval1).unwrap();
        let set2 = buckets2.get_mut(&hashval2).unwrap();
        set1.insert(in_str);
        set2.insert(in_str);

        let intersect: HashSet<&&str> = set1.intersection(set2).collect();
        if intersect.len() != 1 {
            return Err("|set1 - set2 intersection| > 1".to_string());
        }
    }

    let mut table1 = Vec::<u16>::with_capacity(hashlen);
    table1.resize(hashlen, u16::MAX);
    let mut table2 = Vec::<u16>::with_capacity(hashlen);
    table2.resize(hashlen, u16::MAX);

    let mut sorted_input = input_strings.to_vec();
    sorted_input.sort();

    let target_ids: HashMap<&str, u16> = sorted_input
        .iter()
        .enumerate()
        .map(|(idx, key)| (*key, idx as u16))
        .collect();
    let mut unassigned_keys: HashSet<&str> = input_strings.iter().copied().collect();
    let mut working_set: HashSet<&str> = HashSet::new();

    // poor man's hypergraph peeling
    while !unassigned_keys.is_empty() {
        let key = *unassigned_keys.iter().next().unwrap();
        unassigned_keys.remove(key);

        working_set.insert(key);
        while !working_set.is_empty() {
            let key = *working_set.iter().next().unwrap();
            working_set.remove(key);
            unassigned_keys.remove(key);

            let hashval1 = hashfunc(key.as_bytes(), param1) % hashlen;
            let hashval2 = hashfunc(key.as_bytes(), param2) % hashlen;
            let set1 = buckets1.get_mut(&hashval1).unwrap();
            let set2 = buckets2.get_mut(&hashval2).unwrap();

            let target_id = target_ids.get(key).unwrap();

            if table1[hashval1] != u16::MAX && table2[hashval2] != u16::MAX {
                return Err(format!("Error: badly chosen has function parameters; input = {key} - {hashval1} - {hashval2} - {set1:?} - {set2:?}"));
            } else if table1[hashval1] != u16::MAX {
                let mut tab2val: i32 = *target_id as i32 - table1[hashval1] as i32;
                if tab2val < 0 {
                    tab2val += datalen as i32;
                }
                table2[hashval2] = tab2val as u16;
            } else if table2[hashval2] != u16::MAX {
                let mut tab1val: i32 = *target_id as i32 - table2[hashval2] as i32;
                if tab1val < 0 {
                    tab1val += datalen as i32;
                }
                table1[hashval1] = tab1val as u16;
            } else {
                table1[hashval1] = *target_id;
                table2[hashval2] = 0;
            }
            set1.remove(key);
            set2.remove(key);

            for other_key in set1.iter() {
                working_set.insert(*other_key);
            }
            for other_key in set2.iter() {
                working_set.insert(*other_key);
            }
        }
    }

    Ok((table1, table2))
}

pub(crate) fn hashfunc(data: &[u8], param: usize) -> usize {
    data.iter().fold(100usize, |acc, val| {
        usize::wrapping_add(usize::wrapping_mul(acc, param), *val as usize)
    })
}
