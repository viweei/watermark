use std::path::Path;
use std::process;

use ab_glyph::{FontVec, PxScale};
use clap::Parser;
use dialoguer::MultiSelect;
use font_kit::source::SystemSource;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;

const VALID_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "tiff"];

/// 为图片批量添加 45 度梅花错位全屏水印
#[derive(Parser, Debug)]
#[command(name = "watermark")]
struct Args {
    /// 水印文字内容（必填）
    text: String,

    /// 输入图片目录（默认: 当前目录）
    #[arg(short = 'i', long = "image", default_value = ".")]
    image_dir: String,

    /// 系统字体名称（默认: Sans）
    #[arg(short = 'f', long = "font", default_value = "方正粗黑宋简体")]
    font_name: String,

    /// 字体大小（默认: 42.0）
    #[arg(short = 's', long = "size", default_value = "45.0")]
    font_size: f32,

    /// 文字颜色，格式 RRGGBBAA 十六进制（默认: 80808050）
    #[arg(short = 'c', long = "color", default_value = "80808050")]
    color: String,
}

fn main() {
    let args = Args::parse();

    // 解析颜色
    let color = match parse_color(&args.color) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ 颜色格式错误: {}。应为 8 位十六进制 RRGGBBAA", e);
            process::exit(1);
        }
    };

    // 1. 扫描图片目录
    let input_dir = Path::new(&args.image_dir);
    if !input_dir.exists() {
        eprintln!("❌ 错误：目录 '{}' 不存在！", input_dir.display());
        process::exit(1);
    }

    let image_files = scan_images(input_dir);
    if image_files.is_empty() {
        eprintln!("❌ 在 '{}' 目录中没有找到任何图片文件。", input_dir.display());
        process::exit(0);
    }

    // 2. 加载系统字体
    let font = match load_system_font(&args.font_name) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ 无法加载字体 '{}': {}", args.font_name, e);
            eprintln!("💡 提示: 请用 fc-list 查看系统中可用字体");
            process::exit(1);
        }
    };

    // 3. 用户交互多选
    let selected = select_images(&image_files);
    if selected.is_empty() {
        println!("❌ 未选择任何图片，程序退出。");
        process::exit(0);
    }

    println!("\n✅ 已勾选 {} 张图片。", selected.len());
    println!("🚀 开始批量添加水印...\n");

    let current_dir = std::env::current_dir().expect("无法获取当前目录");

    for file_name in &selected {
        let input_path = input_dir.join(file_name);
        let output_name = format!("watermarked_{}", file_name);
        let output_path = current_dir.join(&output_name);

        println!("  正在处理: {} -> {}", file_name, output_path.display());

        match add_watermark(
            &input_path,
            &output_path,
            &args.text,
            &font,
            args.font_size,
            color,
        ) {
            Ok(()) => println!("  ✅ 完成: {}", output_name),
            Err(e) => eprintln!("  ❌ 处理失败: {} - {}", file_name, e),
        }
    }

    println!("\n🎉 所有图片处理完毕！");
}

/// 解析十六进制颜色字符串
fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    if s.len() != 8 {
        return Err(format!("需要 8 位十六进制字符，如 '80808050'，实际得到 {} 位", s.len()));
    }
    let r = u8::from_str_radix(&s[0..2], 16)
        .map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&s[2..4], 16)
        .map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&s[4..6], 16)
        .map_err(|e| e.to_string())?;
    let a = u8::from_str_radix(&s[6..8], 16)
        .map_err(|e| e.to_string())?;
    Ok(Rgba([r, g, b, a]))
}

/// 通过 font-kit 从系统中加载指定名称的 TrueType 字体
fn load_system_font(family_name: &str) -> Result<FontVec, String> {
    let source = SystemSource::new();

    // 先通过 family name 查找
    let handle = source
        .select_best_match(
            &[font_kit::family_name::FamilyName::Title(family_name.to_string())],
            &font_kit::properties::Properties::new(),
        )
        .map_err(|e| format!("未找到字体 '{}': {}", family_name, e))?;

    let font_data = match handle {
        font_kit::handle::Handle::Path { path, .. } => {
            std::fs::read(&path).map_err(|e| format!("读取字体失败: {}", e))?
        }
        font_kit::handle::Handle::Memory { bytes, .. } => bytes.to_vec(),
    };

    FontVec::try_from_vec(font_data).map_err(|e| format!("无法解析字体数据: {:?}", e))
}

/// 扫描目录，返回支持的图片文件名列表（已排序）
fn scan_images(dir: &Path) -> Vec<String> {
    let mut images: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if VALID_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            images.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    images.sort();
    images
}

/// 交互式多选图片，返回用户选中的文件名列表
fn select_images(images: &[String]) -> Vec<String> {
    println!("=========================================");
    println!(" 请选择需要添加水印的图片 (可多选):");
    println!(" [↑/↓]: 移动光标  [空格]: 选择/取消  [回车]: 确认");
    println!("=========================================");

    let selections = match MultiSelect::new().items(images).interact() {
        Ok(indices) => indices,
        Err(e) => {
            eprintln!("选择过程出错: {}", e);
            return Vec::new();
        }
    };

    selections.iter().map(|&i| images[i].clone()).collect()
}

/// 45 度梅花错位全屏水印核心算法
fn add_watermark(
    input: &Path,
    output: &Path,
    text: &str,
    font: &FontVec,
    font_size: f32,
    color: Rgba<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 打开原图并转为 RGBA
    let base_img = image::open(input)?.to_rgba8();
    let (img_w, img_h) = base_img.dimensions();

    // 估算文字尺寸
    let scale = PxScale::from(font_size);
    let text_w = estimate_text_width(font_size, text);
    let text_h = (font_size * 1.2) as u32;

    // 动态控制间距
    let col_spacing = text_w + 80;
    let row_spacing = text_h + 40;

    // 创建超大画布，防止边缘留白
    let watermark_size = (std::cmp::max(img_w, img_h) as f32 * 2.0) as u32;
    let mut watermark_layer = RgbaImage::new(watermark_size, watermark_size);

    // 双重循环铺满网格（带奇偶行错位）
    let mut row_idx: u32 = 0;
    let mut y: u32 = 0;
    while y < watermark_size {
        let offset = (row_idx % 2) * (col_spacing / 2);
        let mut x: i32 = -(col_spacing as i32);
        while x < watermark_size as i32 {
            let actual_x = x + offset as i32;
            if actual_x >= 0 {
                draw_text_mut(
                    &mut watermark_layer,
                    color,
                    actual_x,
                    y as i32,
                    scale,
                    font,
                    text,
                );
            }
            x += col_spacing as i32;
        }
        row_idx += 1;
        y += row_spacing;
    }

    // 顺时针旋转 -45 度
    let rotated = rotate_image(&watermark_layer, -45.0);
    let (rot_w, rot_h) = rotated.dimensions();

    // 中央裁剪，与原图尺寸一致
    let crop_x = ((rot_w as f32 - img_w as f32) / 2.0).max(0.0) as u32;
    let crop_y = ((rot_h as f32 - img_h as f32) / 2.0).max(0.0) as u32;
    let crop_w = img_w.min(rot_w.saturating_sub(crop_x));
    let crop_h = img_h.min(rot_h.saturating_sub(crop_y));

    // Alpha 混合：将水印图层叠加到原图上
    let mut result = base_img.clone();
    for y in 0..crop_h {
        for x in 0..crop_w {
            let base = result.get_pixel(x, y);
            let overlay = rotated.get_pixel(crop_x + x, crop_y + y);

            let alpha = overlay[3] as f32 / 255.0;
            let inv_alpha = 1.0 - alpha;

            let r = (base[0] as f32 * inv_alpha + overlay[0] as f32 * alpha) as u8;
            let g = (base[1] as f32 * inv_alpha + overlay[1] as f32 * alpha) as u8;
            let b = (base[2] as f32 * inv_alpha + overlay[2] as f32 * alpha) as u8;

            *result.get_pixel_mut(x, y) = Rgba([r, g, b, 255]);
        }
    }

    // 保存为 JPEG
    let output_img = DynamicImage::ImageRgba8(result);
    output_img.save_with_format(output, ImageFormat::Jpeg)?;

    Ok(())
}

/// 估算文字渲染宽度（CJK 字符约 1em，ASCII 约 0.5em）
fn estimate_text_width(font_size: f32, text: &str) -> u32 {
    let mut total_width = 0.0;
    for c in text.chars() {
        if c.is_ascii() {
            total_width += font_size * 0.5;
        } else {
            total_width += font_size;
        }
    }
    total_width as u32
}

/// 最近邻采样旋转图片（支持任意角度）
fn rotate_image(img: &RgbaImage, angle_deg: f32) -> RgbaImage {
    let angle = angle_deg.to_radians();
    let (w, h) = img.dimensions();
    let cos = angle.cos();
    let sin = angle.sin();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    let new_w = (w as f32 * cos.abs() + h as f32 * sin.abs()) as u32;
    let new_h = (w as f32 * sin.abs() + h as f32 * cos.abs()) as u32;

    let mut rotated = RgbaImage::new(new_w, new_h);
    let new_cx = new_w as f32 / 2.0;
    let new_cy = new_h as f32 / 2.0;

    for y in 0..new_h {
        for x in 0..new_w {
            // 反向映射：从目标坐标计算源坐标
            let src_x = (x as f32 - new_cx) * cos + (y as f32 - new_cy) * sin + cx;
            let src_y = -(x as f32 - new_cx) * sin + (y as f32 - new_cy) * cos + cy;

            let sx = src_x.round() as i32;
            let sy = src_y.round() as i32;

            if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                *rotated.get_pixel_mut(x, y) = *img.get_pixel(sx as u32, sy as u32);
            }
        }
    }

    rotated
}