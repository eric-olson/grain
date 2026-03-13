/// Translates between output offsets and file (input) offsets
/// using the ratio chain of the active processor path.
pub struct OffsetTranslator {
    /// Accumulated ratio: (input_bytes, output_bytes) for the full chain.
    ratio: (usize, usize),
}

impl OffsetTranslator {
    pub fn identity() -> Self {
        Self { ratio: (1, 1) }
    }

    /// Build from a chain of processor ratios (root → leaf order).
    pub fn from_ratios(ratios: &[(usize, usize)]) -> Self {
        let mut input = 1usize;
        let mut output = 1usize;
        for &(ri, ro) in ratios {
            input *= ri;
            output *= ro;
        }
        Self {
            ratio: (input, output),
        }
    }

    /// Convert an output offset to the corresponding file (input) offset.
    #[allow(dead_code)]
    pub fn output_to_input(&self, output_offset: usize) -> usize {
        if self.ratio.1 == 0 {
            return 0;
        }
        output_offset * self.ratio.0 / self.ratio.1
    }

    /// Convert a file (input) offset to the corresponding output offset.
    pub fn input_to_output(&self, input_offset: usize) -> usize {
        if self.ratio.0 == 0 {
            return 0;
        }
        input_offset * self.ratio.1 / self.ratio.0
    }

    /// Compute the output length given an input length.
    pub fn output_len(&self, input_len: usize) -> usize {
        self.input_to_output(input_len)
    }
}
