struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> time_buffer: TimeBuffer;

struct TimeBuffer {
    elapsed_time: f32,
};

// "Heavily inspired" by : https://github.com/gfx-rs/wgpu/blob/trunk/examples/mipmap/src/blit.wgsl
// meant to be called with 3 vertex indices: 0, 1, 2
// draws one large triangle over the clip space like this:
// (the asterisks represent the clip space bounds)
//-1,1           1,1
// ---------------------------------
// |              *              .
// |              *           .
// |              *        .
// |              *      .
// |              *    . 
// |              * .
// |***************
// |            . 1,-1 
// |          .
// |       .
// |     .
// |   .
// |.
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0, 1.0
    );
    result.tex_coords = tc;
    return result;
}

// Badly ported over from: https://www.shadertoy.com/view/ltXczj

const overallSpeed : f32 = 0.2;
const gridSmoothWidth : f32 = 0.015;
const axisWidth : f32 = 0.05;
const majorLineWidth : f32 = 0.025;
const minorLineWidth : f32 = 0.0125;
const majorLineFrequency : f32 = 5.0;
const minorLineFrequency : f32 = 1.0;
const scale : f32 = 5.0;
const lineColor : vec4<f32> = vec4<f32>(0.5, 0.1, 0.7, 1.0);
const minLineWidth : f32 = 0.02;
const maxLineWidth : f32 = 0.5;
const lineSpeed : f32 = 0.5;
const lineAmplitude : f32 = 1.0;
const lineFrequency : f32 = 0.2;
const warpSpeed : f32 = 0.5;
const warpFrequency : f32 = 0.5;
const warpAmplitude : f32 = 1.0;
const offsetFrequency : f32 = 0.5;
const offsetSpeed : f32 = 0.7;
const minOffsetSpread : f32 = 0.6;
const maxOffsetSpread : f32 = 2.0;
const linesPerGroup : i32 = 16;

fn drawCircle(pos : vec2<f32>, radius : f32, coord : vec2<f32>) -> f32 {
    return smoothstep(radius + gridSmoothWidth, radius, length(coord - pos));
}

fn drawSmoothLine(pos : f32, halfWidth : f32, t : f32) -> f32 {
    return smoothstep(halfWidth, 0.0, abs(pos - t));
}

fn drawCrispLine(pos : f32, halfWidth : f32, t : f32) -> f32 {
    return smoothstep(halfWidth + gridSmoothWidth, halfWidth, abs(pos - t));
}

fn drawPeriodicLine(freq : f32, width : f32, t : f32) -> f32 {
    return drawCrispLine(freq / 2.0, width, abs((t % freq) - freq / 2.0));
}

fn drawGridLines(axis : f32) -> f32 {
    return drawCrispLine(0.0, axisWidth, axis)
        + drawPeriodicLine(majorLineFrequency, majorLineWidth, axis)
        + drawPeriodicLine(minorLineFrequency, minorLineWidth, axis);
}

fn drawGrid(space : vec2<f32>) -> f32 {
    return min(1.0, drawGridLines(space.x) + drawGridLines(space.y));
}

fn random(t : f32) -> f32 {
    return (cos(t) + cos(t * 1.3 + 1.3) + cos(t * 1.4 + 1.4)) / 3.0;
}

fn getPlasmaY(x : f32, horizontalFade : f32, offset : f32, iTime: f32) -> f32 {
    return random(x * lineFrequency + iTime * lineSpeed) * horizontalFade * lineAmplitude + offset;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var space : vec2<f32> = input.tex_coords * scale;
    space.y -= 2.0;
    let iTime = time_buffer.elapsed_time;
    let horizontalFade : f32 = 1.0 - (cos(input.tex_coords.x * 6.28) * 0.5 + 0.5);
    let verticalFade : f32 = 1.0 - (cos(input.tex_coords.y * 6.28) * 0.5 + 0.5);

    space.y += random(space.x * warpFrequency + iTime * warpSpeed) * warpAmplitude * (0.5 + horizontalFade);
    space.x += random(space.y * warpFrequency + iTime * warpSpeed + 2.0) * warpAmplitude * horizontalFade;

    var lines : vec4<f32> = vec4<f32>(0.0);

    for (var l : i32 = 0; l < linesPerGroup; l = l + 1) {
        let normalizedLineIndex : f32 = f32(l) / f32(linesPerGroup);
        let offsetTime : f32 = iTime * offsetSpeed;
        let offsetPosition : f32 = f32(l) + space.x * offsetFrequency;
        let rand : f32 = random(offsetPosition + offsetTime) * 0.5 + 0.5;
        let halfWidth : f32 = mix(minLineWidth, maxLineWidth, rand * horizontalFade) / 2.0;
        let offset : f32 = random(offsetPosition + offsetTime * (1.0 + normalizedLineIndex)) * mix(minOffsetSpread, maxOffsetSpread, horizontalFade);
        let linePosition : f32 = getPlasmaY(space.x, horizontalFade, offset, iTime);
        var line : f32 = drawSmoothLine(linePosition, halfWidth, space.y) / 2.0 + drawCrispLine(linePosition, halfWidth * 0.15, space.y);

        let circleX : f32 = ((f32(l) + iTime * lineSpeed) % 25.0) - 12.0;
        let circlePosition : vec2<f32> = vec2<f32>(circleX, getPlasmaY(circleX, horizontalFade, offset, iTime));
        let circle : f32 = drawCircle(circlePosition, 0.01, space) * 4.0;

        line = line + circle;
        lines = lines + line * lineColor * rand;
    }

    return lines;
}
