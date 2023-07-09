## Run the example
```sh
cargo run example_shaders/waves.wgsl
```

### TODO:
- [ ] Pointer stuff
    - [x] Get pointer position
    - [ ] Pass the position to the shader
- [ ] Audio stuff
    - [ ] Record audio via pulseaudio
    - [ ] FFT ( on CPU or GPU )
    - [ ] Make examples using audio
- [ ] Option to choose output. See [this](https://docs.rs/smithay-client-toolkit/latest/smithay_client_toolkit/output/struct.OutputState.html#method.outputs).
- [ ] Custom textures importing
