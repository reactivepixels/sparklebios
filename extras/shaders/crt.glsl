// SparkleBIOS ULTRA: a gentle CRT for Ghostty. Proof of concept.
// Use it:  custom-shader = /absolute/path/to/extras/ultra/crt.glsl   in your Ghostty config, then reload.
// Scanlines, a slight curve, a soft glow on bright text and a vignette. Nothing animates,
// so nothing flickers, and text stays readable at normal font sizes.

const float CURVE = 0.06;       // 0 is flat glass
const float SCANLINE = 0.16;    // 0 is no scanlines
const float GLOW = 0.35;        // bloom on bright pixels
const float VIGNETTE = 0.28;

vec2 curve(vec2 uv) {
    vec2 c = uv * 2.0 - 1.0;
    c *= 1.0 + CURVE * dot(c.yx, c.yx) * vec2(0.55, 1.0);
    return c * 0.5 + 0.5;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = curve(fragCoord / iResolution.xy);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        fragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    vec3 col = texture(iChannel0, uv).rgb;

    // Glow: a small blur of the bright parts, added back.
    vec2 px = 1.5 / iResolution.xy;
    vec3 blur = vec3(0.0);
    for (int x = -2; x <= 2; x++) {
        for (int y = -2; y <= 2; y++) {
            blur += texture(iChannel0, uv + vec2(float(x), float(y)) * px).rgb;
        }
    }
    blur /= 25.0;
    col += GLOW * blur * smoothstep(0.25, 0.9, max(blur.r, max(blur.g, blur.b)));

    // Scanlines locked to screen pixels, every third row darkest.
    float line = 0.5 + 0.5 * cos(fragCoord.y * 2.0943951);
    col *= 1.0 - SCANLINE * (1.0 - line);

    // Vignette.
    vec2 v = uv * (1.0 - uv.yx);
    col *= mix(1.0 - VIGNETTE, 1.0, clamp(pow(v.x * v.y * 18.0, 0.35), 0.0, 1.0));

    fragColor = vec4(col, 1.0);
}
