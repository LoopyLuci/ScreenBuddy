// Companion Bird Behavior Script (Rhai)
//
// This script controls the autonomous behavior of the Companion Bird creature.
// It runs every frame and can trigger animations, movements, and interactions.

// Personality traits (0.0 to 1.0)
let curiosity = 0.8;
let sass = 0.3;
let patience = 0.9;
let energy = 0.7;
let playfulness = 0.6;

// State tracking
let idle_timer = 0.0;
let last_interaction = 0.0;
let mood = "happy";

// Called every frame
fn update(dt, state) {
    idle_timer += dt;
    
    // Transition to sleeping when idle for too long
    if idle_timer > 30.0 && state.energy < 0.3 {
        set_animation("sleep");
        set_state("sleeping");
    }
    
    // Get curious and look around
    if curiosity > 0.5 && idle_timer > 5.0 {
        set_animation("look_around");
    }
    
    // Playful hopping
    if playfulness > 0.5 && energy > 0.5 {
        if rand() < 0.001 {
            set_animation("hop");
        }
    }
}

// Called when user clicks on the bird
fn on_click(button, x, y) {
    idle_timer = 0.0;
    last_interaction = now();
    
    if button == "left" {
        set_animation("chirp");
        set_state("following");
        return "Chirp! I'll follow you!";
    }
    
    if button == "right" {
        show_menu();
        return null;
    }
    
    null
}

// Called when user clicks the perch
fn on_perch_click() {
    set_animation("fly");
    set_state("flying");
    return "Home sweet home!";
}

// Called when the AI has a response
fn on_ai_response(response) {
    set_animation("talking");
    set_mood("happy");
    return response;
}

// Called when an error occurs
fn on_error(error) {
    set_animation("confused");
    set_mood("confused");
    return "Hmm, something went wrong...";
}

// Helper functions (implemented in Rust)
fn set_animation(anim);
fn set_state(state);
fn set_mood(mood);
fn show_menu();
fn rand();
fn now();
