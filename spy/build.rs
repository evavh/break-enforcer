use std::fs;
use std::mem::size_of;
use std::path::Path;
use std::{env, path::PathBuf};

fn main() {
    // // build directory for this crate
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let ass_loop = loop_assembly().into_bytes();
    std::fs::write(dbg!(out_dir.join("loop.s")), ass_loop)
        .expect("failed to write assembly loop to file");
}

fn loop_assembly() -> String {
    let sections: Vec<String> = (4..array_length())
        .map(|i| i*size_of::<u32>())
        .map(|offset| {
            format!(
                "//load the current value of the pin into r1
        ldr r1, [r0]                              // 2 cycles
        // store r1 in ARRAY[n]
        str r1, [r2, #{}]                         // 2 cycles
        NOP                                       // 1 cycle
        NOP                                       // 1 cycle
        NOP                                       // 1 cycle
        // = n*7 cycles after first read
    ",
                offset
            )
        })
        .collect();
    sections.iter().map(String::as_str).collect()
}

fn array_length() -> usize {
    // let main = Path::new(env!("CARGO_MANIFEST_DIR"))
    //     .join("src")
    //     .join("main.rs");
    // let main = fs::read_to_string(main).unwrap();
    // let arr_start = main.find("ARRAY").expect("main missing static ARRAY");
    // let semicolon = arr_start
    //     + main[arr_start..]
    //         .find(";")
    //         .expect("ARRAY type missing ';'");
    // let closingbr = semicolon
    //     + main[semicolon..]
    //         .find("]")
    //         .expect("ARRAY type closing ']'");
    // main[semicolon+1..closingbr].trim().parse().expect(
    //     "build script needs ARRAY length should be a constant, \
    //     if you need to access it a as a constant use: `unsafe{ ARRAY.len() }",
    // )
    6
}
