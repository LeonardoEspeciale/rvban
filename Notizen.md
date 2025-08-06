# Audio Framework

## ALSA

### Frames

A frame is the unity of one sample, regardless of the number of channels or the bitwidth. A 16 bit stereo frame consists of 4 bytes, whereas a 32 bit 5.1 frame consists of 24 bytes. 

## Pipewire

- This has to be created entirely from scratch

# Codecs

## OPUS

- Implement forward error correction (FEC)
- Deinit decoder when not needed anymore


# VBAN 

- Check again: `let mut to_sink = vec![0; audio_data.len() / bits_per_sample as usize];` 
    - bits_per_sample is defined as `let bits_per_sample = VBAN_BIT_RESOLUTION_SIZE[self.sample_format.unwrap() as usize];`
    - sample_format is treated as usize -> where is defined how to convet sample_format into usize?? -> Try with different sample foirmats bigger than 2 bytes
- VBAN specification says that the maximum length of a packet must be 1436 due to the limitations of the IP spec. This program uses a much higher value calculated from the maximuim amount of samples