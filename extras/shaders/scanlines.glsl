// SparkleBIOS: scanlines for Ghostty. Nothing else: no curve, no glow, no vignette.
// Use it:  custom-shader = /absolute/path/to/extras/shaders/scanlines.glsl   in your Ghostty config, then reload.
// Every third pixel row is darkened a little. Nothing animates, so nothing flickers.

const float STRENGTH = 0.14;   // 0 is off, 0.3 is heavy
const float PERIOD = 3.0;      // pixel rows per scanline

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec3 col = texture(iChannel0, fragCoord / iResolution.xy).rgb;
    float line = 0.5 + 0.5 * cos(fragCoord.y * 6.2831853 / PERIOD);
    col *= 1.0 - STRENGTH * (1.0 - line);
    fragColor = vec4(col, 1.0);
}
