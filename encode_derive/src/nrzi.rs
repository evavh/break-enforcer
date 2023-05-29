pub type Bit = u8;

// nszi-s: logical 0 is a transition, logical 1 no transition
pub fn encode(prev: Bit, data: &[Bit]) -> Vec<Bit> {
    let mut state = prev;
    data.iter()
        .map(|b| {
            if *b == 1 {
                // stay
                state
            } else {
                // change high state
                state ^= 1;
                state
            }
        })
        .collect()
}

// verify using:
// http://www.ee.unb.ca/cgi-bin/tervo/encoding.pl?binary=111&c=1&d=1
#[cfg(test)]
mod tests {
    use super::*;

    mod prev_one {
        use super::*;

        #[test]
        fn ones() {
            let input = [1, 1, 1];
            let state = 1;
            let out = encode(state, &input);

            assert_eq!(&out, &[1, 1, 1])
        }

        #[test]
        fn zeros() {
            let input = [0, 0, 0];
            let state = 1;
            let out = encode(state, &input);

            assert_eq!(&out, &[0, 1, 0])
        }
    }

    mod prev_zero {
        use super::*;

        #[test]
        fn ones() {
            let input = [1, 1, 1];
            let state = 0;
            let out = encode(state, &input);

            assert_eq!(&out, &[0, 0, 0])
        }

        #[test]
        fn zeros() {
            let input = [0, 0, 0];
            let state = 0;
            let out = encode(state, &input);

            assert_eq!(&out, &[1, 0, 1])
        }
    }

    #[test]
    fn wiki_nrzi_example() {
        // https://en.wikipedia.org/wiki/Non-return-to-zero#NRZI
        let input = [1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 1, 0];
        let state = 1;
        let out = encode(state, &input);

        assert_eq!(&out, &[1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 0, 1])
    }

    #[test]
    fn wiki_example_nack() {
        let input = [0,1,0,1,1,0,1,0]; // ack
        let state = 1; // always a one (preamble ends with one)
        let out = encode(state, &input);

        assert_eq!(&out, &[0,0,1,1,1,0,0,1])
    }

    #[test]
    fn wiki_example_ack() {
        let input = [0,1,0,0,1,0,1,1]; // ack
        let state = 1; // always a one (preamble ends with one)
        let out = encode(state, &input);

        assert_eq!(&out, &[0,0,1,0,0,1,1,1])
    }
}
