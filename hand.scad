// Parametric Human Hand
// Units: mm  |  Default scale ≈ adult medium
// Print orientation: palm flat on bed, fingers pointing up

/* [Hand Scale] */
hand_scale = 1.0;   // 0.5 = child, 1.2 = large adult

/* [Finger Curl Angles — degrees (0 = straight, 90 = fully bent)] */
// Each is [proximal_knuckle, middle_knuckle, distal_knuckle]
index_curl  = [0, 0, 0];
middle_curl = [0, 0, 0];
ring_curl   = [0, 0, 0];
pinky_curl  = [0, 0, 0];
// Thumb: [proximal_knuckle, distal_knuckle]
thumb_curl  = [0, 0];

/* [Hidden] */
$fn      = 32;
JFACETS  = 20;   // facets for joint spheres

// ── Palm ─────────────────────────────────────────────────────────
PALM_W  = 80;   // wrist width
PALM_H  = 72;   // height wrist→knuckles
PALM_T  = 22;   // dorsal–palmar thickness
KNUCK_W = 0.88; // knuckle row is this fraction of wrist width

// ── Fingers: [total_len, base_width, palm_x_offset, splay_deg] ──
FDATA = [
    [74, 18, -27, -3],   // index
    [84, 19,  -9,  0],   // middle
    [79, 17,  10,  2],   // ring
    [62, 14,  26,  5],   // pinky
];
SEG_R = [0.34, 0.35, 0.31];  // proximal : middle : distal length ratios

// ── Thumb ────────────────────────────────────────────────────────
THUMB_LEN  = 60;
THUMB_W    = 22;
THUMB_SEGR = [0.44, 0.56]; // proximal : distal

// ═══════════════════════════════════════════════════════════════════

// Rounded-box pill along +Y, from y=0 to y=len
module seg_body(len, w) {
    t  = w * 0.72;   // dorsal–palmar thickness
    cr = w * 0.13;   // corner radius
    hull()
        for (x = [-w/2+cr, w/2-cr])
            for (y = [cr, len-cr])
                for (z = [-t/2+cr, t/2-cr])
                    translate([x, y, z]) sphere(r=cr, $fn=8);
}

// Knuckle sphere
module knob(r) { sphere(r=r, $fn=JFACETS); }

// Kinematic segment chain.
// lens / widths / curls are same-length vectors; i = current index.
module chain(lens, widths, curls, i=0) {
    if (i < len(lens)) {
        c = (i < len(curls)) ? curls[i] : 0;
        l = lens[i];
        w = widths[i];
        rotate([-c, 0, 0]) {
            knob(w * 0.47);
            seg_body(l, w);
            if (i + 1 < len(lens))
                translate([0, l, 0])
                    chain(lens, widths, curls, i+1);
            else
                // Rounded fingertip cap at far end of last segment
                translate([0, l, 0]) sphere(r=w*0.38, $fn=JFACETS);
        }
    }
}

module finger(total_len, base_w, curls) {
    ws = [base_w, base_w*0.87, base_w*0.73];
    ls = [total_len*SEG_R[0], total_len*SEG_R[1], total_len*SEG_R[2]];
    chain(ls, ws, curls);
}

module thumb(total_len, base_w, curls) {
    ws = [base_w, base_w*0.78];
    ls = [total_len*THUMB_SEGR[0], total_len*THUMB_SEGR[1]];
    chain(ls, ws, curls);
}

module palm_body() {
    kw = PALM_W * KNUCK_W;
    rb = 9;  rt = 7;
    hull() {
        // Wrist row (bottom)
        for (x = [-PALM_W/2+rb, PALM_W/2-rb])
            translate([x, 0, 0])
                cylinder(h=PALM_T, r=rb, center=true, $fn=20);
        // Knuckle row (top, slightly thinner)
        for (x = [-kw/2+rt, kw/2-rt])
            translate([x, PALM_H, 0])
                cylinder(h=PALM_T*0.80, r=rt, center=true, $fn=20);
    }
}

module wrist_body() {
    // Tapers outward slightly toward the forearm stub
    hull() {
        for (x = [-PALM_W/2+9, PALM_W/2-9])
            translate([x, 0, 0])
                cylinder(h=PALM_T, r=9, center=true, $fn=20);
        for (x = [-PALM_W/2+11, PALM_W/2-11])
            translate([x, -32, 0])
                cylinder(h=PALM_T*0.74, r=11, center=true, $fn=20);
    }
}

module hand() {
    scale([hand_scale, hand_scale, hand_scale]) {
        wrist_body();
        palm_body();

        // Four fingers
        all_curls = [index_curl, middle_curl, ring_curl, pinky_curl];
        for (i = [0:3]) {
            d = FDATA[i];
            translate([d[2], PALM_H - 5, 0])
                rotate([0, 0, d[3]])
                    finger(d[0], d[1], all_curls[i]);
        }

        // Thumb — lateral side, angled out from the palm plane
        translate([-PALM_W/2 + 3, PALM_H * 0.40, 0])
            rotate([0, -15, -42])
                thumb(THUMB_LEN, THUMB_W, thumb_curl);
    }
}

hand();
