fn main() {
    let window_size = 3;
    let values = vec![Some(10.0), Some(9.0), Some(8.0), Some(10.0)];
    let max_close_vec = vec![None, None, Some(10.0), Some(10.0)];

    for i in 0..values.len() {
        if i < window_size - 1 {
            continue;
        }

        let mut sum_sq_drawdown = 0.0;
        let mut all_valid = true;

        for j in (i + 1 - window_size)..=i {
            if let (Some(val), Some(max_val)) = (values[j], max_close_vec[j]) {
                println!("j={}, val={}, max_val={}", j, val, max_val);
            } else {
                all_valid = false;
                println!("Invalid at j={}", j);
            }
        }
        println!("all_valid for i={} is {}", i, all_valid);
    }
}
