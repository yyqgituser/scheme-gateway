const TAG_INT: i8 = 0;
const TAG_BOOL: i8 = 1;

/// Called from JIT-compiled code to print a tagged value.
#[no_mangle]
pub extern "C" fn rt_print_value(tag: i8, payload: i64) {
    match tag {
        TAG_INT => println!("{}", payload),
        TAG_BOOL => {
            if payload != 0 {
                println!("#t");
            } else {
                println!("#f");
            }
        }
        _ => println!("nil"),
    }
}
