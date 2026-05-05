use crate::diagram::StyleProps;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ThemePalette {
    pub n1: [u8; 3],
    pub n2: [u8; 3],
    pub n3: [u8; 3],
    pub n4: [u8; 3],
    pub n5: [u8; 3],
    pub n6: [u8; 3],
    pub n7: [u8; 3],
    pub b1: [u8; 3],
    pub b2: [u8; 3],
    pub b3: [u8; 3],
    pub b4: [u8; 3],
    pub b5: [u8; 3],
    pub b6: [u8; 3],
    pub aa2: [u8; 3],
    pub aa4: [u8; 3],
    pub aa5: [u8; 3],
    pub ab4: [u8; 3],
    pub ab5: [u8; 3],
}

pub fn get_theme(id: i32) -> ThemePalette {
    match id {
        0 => neutral_default(),
        1 => neutral_grey(),
        3 => flagship_terrastruct(),
        200 => dark_mauve(),
        300 => terminal(),
        _ => neutral_default(),
    }
}

fn neutral_default() -> ThemePalette {
    ThemePalette {
        n1: [10, 15, 37],
        n2: [103, 108, 126],
        n3: [148, 153, 171],
        n4: [207, 210, 221],
        n5: [222, 225, 235],
        n6: [238, 241, 248],
        n7: [255, 255, 255],
        b1: [13, 50, 178],
        b2: [13, 50, 178],
        b3: [72, 108, 223],
        b4: [130, 162, 244],
        b5: [186, 204, 249],
        b6: [225, 234, 253],
        aa2: [75, 94, 178],
        aa4: [165, 178, 228],
        aa5: [210, 218, 242],
        ab4: [150, 170, 210],
        ab5: [200, 215, 240],
    }
}

fn neutral_grey() -> ThemePalette {
    ThemePalette {
        n1: [10, 15, 37],
        n2: [103, 108, 126],
        n3: [148, 153, 171],
        n4: [207, 210, 221],
        n5: [222, 225, 235],
        n6: [238, 241, 248],
        n7: [255, 255, 255],
        b1: [205, 214, 244],
        b2: [205, 214, 244],
        b3: [180, 190, 220],
        b4: [200, 210, 235],
        b5: [220, 228, 245],
        b6: [240, 243, 250],
        aa2: [170, 180, 210],
        aa4: [200, 210, 235],
        aa5: [225, 232, 248],
        ab4: [190, 200, 225],
        ab5: [215, 222, 242],
    }
}

fn flagship_terrastruct() -> ThemePalette {
    ThemePalette {
        n1: [10, 15, 37],
        n2: [103, 108, 126],
        n3: [148, 153, 171],
        n4: [207, 210, 221],
        n5: [222, 225, 235],
        n6: [238, 241, 248],
        n7: [255, 255, 255],
        b1: [0, 14, 61],
        b2: [35, 76, 218],
        b3: [72, 108, 223],
        b4: [130, 162, 244],
        b5: [186, 204, 249],
        b6: [225, 234, 253],
        aa2: [75, 94, 178],
        aa4: [165, 178, 228],
        aa5: [210, 218, 242],
        ab4: [150, 170, 210],
        ab5: [200, 215, 240],
    }
}

fn dark_mauve() -> ThemePalette {
    ThemePalette {
        n1: [205, 214, 244],
        n2: [186, 194, 222],
        n3: [166, 173, 200],
        n4: [108, 112, 134],
        n5: [88, 91, 112],
        n6: [69, 71, 90],
        n7: [30, 30, 46],
        b1: [203, 166, 247],
        b2: [137, 180, 250],
        b3: [116, 199, 236],
        b4: [148, 226, 213],
        b5: [166, 227, 161],
        b6: [69, 71, 90],
        aa2: [245, 194, 231],
        aa4: [242, 205, 205],
        aa5: [250, 179, 135],
        ab4: [249, 226, 175],
        ab5: [245, 224, 220],
    }
}

fn terminal() -> ThemePalette {
    ThemePalette {
        n1: [0, 255, 0],
        n2: [0, 200, 0],
        n3: [0, 160, 0],
        n4: [0, 120, 0],
        n5: [0, 80, 0],
        n6: [0, 40, 0],
        n7: [0, 0, 0],
        b1: [0, 255, 0],
        b2: [0, 200, 0],
        b3: [0, 160, 0],
        b4: [0, 120, 0],
        b5: [0, 80, 0],
        b6: [0, 20, 0],
        aa2: [0, 180, 0],
        aa4: [0, 140, 0],
        aa5: [0, 100, 0],
        ab4: [0, 150, 0],
        ab5: [0, 110, 0],
    }
}

pub fn apply_theme_to_node(style: &mut StyleProps, palette: &ThemePalette, is_container: bool) {
    if style.fill.is_none() {
        style.fill = Some(if is_container { palette.n7 } else { palette.b6 });
    }
    if style.stroke.is_none() {
        style.stroke = Some(palette.b1);
    }
    if style.color.is_none() {
        style.color = Some(palette.n1);
    }
}

pub fn apply_theme_to_edge(style: &mut StyleProps, palette: &ThemePalette) {
    if style.stroke.is_none() {
        style.stroke = Some(palette.b1);
    }
    if style.color.is_none() {
        style.color = Some(palette.n2);
    }
}
