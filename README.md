## Run the example
```sh
cargo run example_shaders/waves.wgsl
```

### TODO:
- [x] Pointer stuff
    - [x] Get pointer position
    - [x] Pass the position to the shader
- [ ] Audio stuff
    - [x] Record audio via pulseaudio
    - [x] FFT on CPU 
    - [ ] FFT on GPU 
    - [ ] Make examples using audio
- [x] Option to choose output. See [this](https://docs.rs/smithay-client-toolkit/latest/smithay_client_toolkit/output/struct.OutputState.html#method.outputs).
- [ ] Custom textures importing
