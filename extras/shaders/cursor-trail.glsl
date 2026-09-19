// Rainbows and Unicorns: cursor trail, a custom shader for Ghostty.
//
// When the cursor jumps, it leaves a short six-stripe trail from where it was
// to where it is now. The trail tapers towards the tail, retracts into the
// cursor and is gone in about a third of a second. Typing one character at a
// time barely shows it; jumping across the line or the screen does.
//
//   custom-shader = /path/to/cursor-trail.glsl
//
// Needs a Ghostty recent enough to provide the cursor uniforms used below
// (iCurrentCursor, iPreviousCursor, iTimeCursorChange).

const float DURATION = 0.35;   // seconds until the trail has fully retracted
const float OPACITY  = 0.80;   // peak strength of the trail
const float TAIL     = 0.30;   // width of the tail as a fraction of the cursor height
const float MIN_JUMP = 1.5;    // jumps shorter than this many cursor widths leave no trail

vec3 stripe(float v) {
    // The 1977 order, top to bottom.
    if (v < 1.0 / 6.0) return vec3(0.384, 0.733, 0.278); // green
    if (v < 2.0 / 6.0) return vec3(0.988, 0.722, 0.153); // yellow
    if (v < 3.0 / 6.0) return vec3(0.961, 0.510, 0.122); // orange
    if (v < 4.0 / 6.0) return vec3(0.878, 0.271, 0.247); // red
    if (v < 5.0 / 6.0) return vec3(0.733, 0.384, 0.753); // purple
    return vec3(0.114, 0.608, 0.890);                    // blue
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    fragColor = base;

    vec2 size = iCurrentCursor.zw;
    // Cursor rectangles are given by their top left corner, with y pointing up.
    vec2 from = iPreviousCursor.xy + vec2(size.x * 0.5, -size.y * 0.5);
    vec2 to   = iCurrentCursor.xy  + vec2(size.x * 0.5, -size.y * 0.5);

    float age = (iTime - iTimeCursorChange) / DURATION;
    vec2 path = to - from;
    float len = length(path);
    if (age < 0.0 || age >= 1.0 || len < size.x * MIN_JUMP) return;

    vec2 dir = path / len;
    vec2 nrm = vec2(-dir.y, dir.x);
    vec2 rel = fragCoord - from;
    float along = dot(rel, dir) / len;   // 0 at the tail, 1 at the cursor
    float across = dot(rel, nrm);        // signed distance from the centre line

    // The tail end chases the cursor as the trail ages.
    if (along < age || along > 1.0) return;

    float half_width = 0.5 * size.y * mix(TAIL, 1.0, along);
    if (abs(across) > half_width) return;

    float band = clamp(0.5 - across / (2.0 * half_width), 0.0, 0.999);
    float fade = (1.0 - age) * smoothstep(age, 1.0, along);
    fragColor = vec4(mix(base.rgb, stripe(band), OPACITY * fade), base.a);
}
