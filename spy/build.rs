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
    let mask = 0b0000_0111;
    let len = array_length();
    assert!(len >= 5, "array must be a minimum of 5 long");
    let sections: Vec<String> = (5..array_length())
        .map(|i| i + size_of::<u32>())
        .map(|offset| {
            assert!(
                offset < 4095,
                "assembly can only handle arrays up to 4094 bytes"
            );
            format!(
                "//load the current value of the pin into r1
        ldr r1, [r0]                              // 2 cycles
        // store r1 in ARRAY[n]
        strb r1, [r2, #{offset}]                   // 2 cycles
        TST r1, #{mask:x}                         // 1 cycle
        // see Bcc in the reference where cc is a condition code
        // such as EQ which means Z flag is set. The Z flag is set 
        // by TST if r1 & mask was zero.
        BEQ .EXIT_READ_PACKETS                    // 1 cycle (if not breaking)
        // update length of data r3
        ADD r3, r3, 1                             // 1 cycle
        // = n*7 cycles after first read
    "
            )
        })
        .collect();
    sections.iter().map(String::as_str).collect()
}

fn array_length() -> usize {
    let main = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("main.rs");
    let main = fs::read_to_string(main).unwrap();
    let arr_start = main.find("ARRAY_LEN").expect("main missing static ARRAY");
    let equals = arr_start + main[arr_start..].find("=").expect("ARRAY_LEN missing '='");
    let semicolon = equals
        + main[equals..]
            .find(";")
            .expect("ARRAY_LEN not ending with ';'");
    main[equals + 1..semicolon]
        .trim()
        .replace("_", "")
        .parse()
        .expect("build script needs ARRAY_LEN constant")
}
