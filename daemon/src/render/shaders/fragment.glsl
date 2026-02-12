#version 100
precision mediump float;

varying vec2 v_texcoord;

uniform sampler2D u_texture;
uniform sampler2D u_texture_prev;
uniform float u_progress;      // Transition progress 0.0 to 1.0
uniform int u_transition_type; // 0=none, 1=fade, 2=slide_left, 3=slide_right, 4=slide_up, 5=slide_down

void main() {
    vec4 current_color = texture2D(u_texture, v_texcoord);

    // No transition or transition complete
    if (u_transition_type == 0 || u_progress >= 1.0) {
        gl_FragColor = current_color;
        return;
    }

    vec4 prev_color = texture2D(u_texture_prev, v_texcoord);

    // Fade transition
    if (u_transition_type == 1) {
        gl_FragColor = mix(prev_color, current_color, u_progress);
        return;
    }

    // Slide transitions
    vec2 offset = vec2(0.0);
    float p = 1.0 - u_progress;

    if (u_transition_type == 2) { // slide left
        offset = vec2(p, 0.0);
    } else if (u_transition_type == 3) { // slide right
        offset = vec2(-p, 0.0);
    } else if (u_transition_type == 4) { // slide up
        offset = vec2(0.0, -p);
    } else if (u_transition_type == 5) { // slide down
        offset = vec2(0.0, p);
    }

    vec2 current_coord = v_texcoord - offset;
    vec2 prev_coord = v_texcoord + (vec2(1.0) - abs(offset)) * sign(offset);

    if (current_coord.x >= 0.0 && current_coord.x <= 1.0 &&
        current_coord.y >= 0.0 && current_coord.y <= 1.0) {
        gl_FragColor = texture2D(u_texture, current_coord);
    } else if (prev_coord.x >= 0.0 && prev_coord.x <= 1.0 &&
               prev_coord.y >= 0.0 && prev_coord.y <= 1.0) {
        gl_FragColor = texture2D(u_texture_prev, prev_coord);
    } else {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
    }
}
