use std::f64;

fn main() {
    let window_size = 3;
    let values = vec![Some(10.0), Some(9.0), Some(8.0), Some(10.0)];
    let mut deque: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    let mut result = Vec::new();

    for i in 0..values.len() {
        while let Some(&front) = deque.front() {
            if front + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = values[i] {
            while let Some(&back) = deque.back() {
                if let Some(back_val) = values[back] {
                    if back_val <= val {
                        deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    deque.pop_back();
                }
            }
            deque.push_back(i);
        }

        if i >= window_size - 1 {
            if let Some(&front) = deque.front() {
                result.push(values[front]);
            } else {
                result.push(None);
            }
        } else {
            result.push(None);
        }
    }

    println!("{:?}", result);
}
