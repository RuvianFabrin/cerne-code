//! Computer use por GRADE: em vez de pedir ao modelo um pixel (que modelos locais erram), o screenshot sai com
//! uma grade numerada por cima. O modelo escolhe o NÚMERO da célula; o código calcula o centro dela e move o
//! mouse. Se o alvo é pequeno, o modelo pede uma grade nova só dentro de uma célula (refinamento).
//!
//! Tudo aqui é lógica pura (geometria, desenho e estado da última grade), sem captura de tela nem mouse: isso
//! fica em `computer_engine.rs`. Assim a matemática dos cliques tem teste sem precisar de monitor.

use anyhow::{anyhow, Result};
use image::RgbaImage;
use std::sync::Mutex;

/// Grade da tela inteira: células de ~190x180 px num monitor 1920x1080 (60 células).
pub const COLUNAS_PADRAO: u32 = 10;
pub const LINHAS_PADRAO: u32 = 6;
/// Grade dentro de uma célula já escolhida (refinamento): 24 células.
pub const COLUNAS_REFINO: u32 = 6;
pub const LINHAS_REFINO: u32 = 4;
pub const DIM_MIN: u32 = 2;
pub const DIM_MAX: u32 = 20;
/// Recorte menor que isso (maior lado) é ampliado antes de desenhar, para os números ficarem legíveis.
pub const LADO_MINIMO_DO_RECORTE: u32 = 800;

/// Retângulo em pixels da imagem capturada (não da tela lógica).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Retangulo {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A última grade mostrada ao modelo: o suficiente para transformar "célula N" em coordenada de clique sem
/// capturar a tela de novo.
#[derive(Clone, Debug, PartialEq)]
pub struct GradeAtual {
    /// Tamanho da imagem capturada, em pixels físicos.
    pub imagem_w: u32,
    pub imagem_h: u32,
    /// Monitor em coordenadas de clique (pode ser menor que a imagem quando a tela tem escala de DPI).
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub monitor_w: u32,
    pub monitor_h: u32,
    /// Trecho da imagem que a grade cobre: a tela inteira ou uma célula ampliada.
    pub regiao: Retangulo,
    pub colunas: u32,
    pub linhas: u32,
}

/// Quantidade de colunas/linhas pedida pelo modelo, limitada a um intervalo que ainda dá para ler.
pub fn dimensao(pedida: Option<i64>, padrao: u32) -> u32 {
    match pedida {
        Some(v) if v >= 0 => (v as u32).clamp(DIM_MIN, DIM_MAX),
        _ => padrao,
    }
}

impl GradeAtual {
    pub fn total(&self) -> u32 {
        self.colunas * self.linhas
    }

    /// Retângulo (em pixels da imagem) da célula `n`, numerada de 1, linha a linha a partir do canto
    /// superior esquerdo: 1 é a primeira coluna da primeira linha.
    pub fn retangulo_da_celula(&self, n: i64) -> Result<Retangulo> {
        if n < 1 || n as u64 > self.total() as u64 {
            return Err(anyhow!("a célula {n} não existe: a grade atual vai de 1 a {} ({} colunas x {} linhas)", self.total(), self.colunas, self.linhas));
        }
        let i = (n - 1) as u32;
        let (linha, coluna) = (i / self.colunas, i % self.colunas);
        let w = self.regiao.w / self.colunas as f64;
        let h = self.regiao.h / self.linhas as f64;
        Ok(Retangulo { x: self.regiao.x + coluna as f64 * w, y: self.regiao.y + linha as f64 * h, w, h })
    }

    /// Centro da célula `n` em coordenadas de clique (as mesmas de `computer_use_click`).
    pub fn centro_na_tela(&self, n: i64) -> Result<(i32, i32)> {
        let r = self.retangulo_da_celula(n)?;
        let cx = r.x + r.w / 2.0;
        let cy = r.y + r.h / 2.0;
        let x = self.monitor_x as f64 + cx * self.monitor_w as f64 / self.imagem_w.max(1) as f64;
        let y = self.monitor_y as f64 + cy * self.monitor_h as f64 / self.imagem_h.max(1) as f64;
        Ok((x.round() as i32, y.round() as i32))
    }

    /// Grade nova só dentro da célula `n` (refinamento).
    pub fn refinar(&self, n: i64, colunas: u32, linhas: u32) -> Result<GradeAtual> {
        let regiao = self.retangulo_da_celula(n)?;
        Ok(GradeAtual { regiao, colunas, linhas, ..self.clone() })
    }

    /// Recorte inteiro (x, y, largura, altura) dentro da imagem, com pelo menos 1 pixel.
    pub fn recorte(&self) -> (u32, u32, u32, u32) {
        let x = (self.regiao.x.floor().max(0.0) as u32).min(self.imagem_w.saturating_sub(1));
        let y = (self.regiao.y.floor().max(0.0) as u32).min(self.imagem_h.saturating_sub(1));
        let w = (self.regiao.w.round().max(1.0) as u32).min(self.imagem_w - x);
        let h = (self.regiao.h.round().max(1.0) as u32).min(self.imagem_h - y);
        (x, y, w.max(1), h.max(1))
    }
}

static ULTIMA: Mutex<Option<GradeAtual>> = Mutex::new(None);

pub fn guardar(g: GradeAtual) {
    if let Ok(mut u) = ULTIMA.lock() {
        *u = Some(g);
    }
}

pub fn atual() -> Option<GradeAtual> {
    ULTIMA.lock().ok().and_then(|u| u.clone())
}

pub fn limpar() {
    if let Ok(mut u) = ULTIMA.lock() {
        *u = None;
    }
}

// Cores fora do comum nas interfaces: linhas magenta finas e semitransparentes; número amarelo sobre uma
// tarjeta escura, para ler em fundo claro e escuro sem cobrir o conteúdo.
const COR_LINHA: [u8; 3] = [255, 0, 200];
const ALFA_LINHA: f32 = 0.45;
const COR_TARJETA: [u8; 3] = [0, 0, 0];
const ALFA_TARJETA: f32 = 0.72;
const COR_NUMERO: [u8; 3] = [255, 235, 0];

/// Fonte 3x5 dos dígitos: cada linha tem 3 bits (o mais à esquerda é o bit 2).
const DIGITOS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn misturar(img: &mut RgbaImage, x: i64, y: i64, cor: [u8; 3], alfa: f32) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    for c in 0..3 {
        p.0[c] = (p.0[c] as f32 * (1.0 - alfa) + cor[c] as f32 * alfa).round() as u8;
    }
    p.0[3] = 255;
}

/// Tamanho de cada "pixel" do dígito: cresce com a célula, dentro de 1 a 4.
pub fn escala_do_numero(celula_w: f64, celula_h: f64) -> u32 {
    ((celula_h / 26.0).min(celula_w / 22.0).floor() as u32).clamp(1, 4)
}

fn desenhar_numero(img: &mut RgbaImage, x0: i64, y0: i64, numero: u32, escala: u32) {
    let texto = numero.to_string();
    let esc = escala as i64;
    let largura = texto.len() as i64 * 4 * esc + esc; // 3 colunas + 1 de espaço por dígito
    let altura = 5 * esc + 2 * esc;
    for dy in 0..altura {
        for dx in 0..largura {
            misturar(img, x0 + dx, y0 + dy, COR_TARJETA, ALFA_TARJETA);
        }
    }
    for (i, ch) in texto.chars().enumerate() {
        let d = ch.to_digit(10).unwrap_or(0) as usize;
        for (linha, bits) in DIGITOS[d].iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for sy in 0..esc {
                        for sx in 0..esc {
                            let px = x0 + esc + (i as i64 * 4 + col) * esc + sx;
                            let py = y0 + esc + linha as i64 * esc + sy;
                            misturar(img, px, py, COR_NUMERO, 1.0);
                        }
                    }
                }
            }
        }
    }
}

/// Desenha a grade `colunas` x `linhas` sobre a imagem inteira: linhas finas entre as células e o número de
/// cada célula no canto superior esquerdo dela (1 a colunas*linhas, linha a linha).
pub fn desenhar_grade(img: &mut RgbaImage, colunas: u32, linhas: u32) {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let cw = w / colunas as f64;
    let ch = h / linhas as f64;
    for c in 1..colunas {
        let x = (c as f64 * cw).round() as i64;
        for y in 0..img.height() as i64 {
            misturar(img, x, y, COR_LINHA, ALFA_LINHA);
        }
    }
    for l in 1..linhas {
        let y = (l as f64 * ch).round() as i64;
        for x in 0..img.width() as i64 {
            misturar(img, x, y, COR_LINHA, ALFA_LINHA);
        }
    }
    let escala = escala_do_numero(cw, ch);
    for l in 0..linhas {
        for c in 0..colunas {
            let x0 = (c as f64 * cw).round() as i64 + 2;
            let y0 = (l as f64 * ch).round() as i64 + 2;
            desenhar_numero(img, x0, y0, l * colunas + c + 1, escala);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn tela(colunas: u32, linhas: u32) -> GradeAtual {
        // Monitor 1920x1080 em (0,0), imagem do mesmo tamanho.
        GradeAtual {
            imagem_w: 1920,
            imagem_h: 1080,
            monitor_x: 0,
            monitor_y: 0,
            monitor_w: 1920,
            monitor_h: 1080,
            regiao: Retangulo { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 },
            colunas,
            linhas,
        }
    }

    #[test]
    fn cells_are_numbered_row_by_row_from_the_top_left() {
        let g = tela(10, 6);
        assert_eq!(g.total(), 60);
        let primeira = g.retangulo_da_celula(1).unwrap();
        assert_eq!((primeira.x, primeira.y, primeira.w, primeira.h), (0.0, 0.0, 192.0, 180.0));
        // A 11 é a primeira coluna da segunda linha; a 60 é a última coluna da última linha.
        let onze = g.retangulo_da_celula(11).unwrap();
        assert_eq!((onze.x, onze.y), (0.0, 180.0));
        let ultima = g.retangulo_da_celula(60).unwrap();
        assert_eq!((ultima.x, ultima.y), (1728.0, 900.0));
    }

    #[test]
    fn a_cell_outside_the_grid_is_refused_with_the_valid_range() {
        let g = tela(10, 6);
        for n in [0, -3, 61, 1000] {
            let e = g.retangulo_da_celula(n).unwrap_err().to_string();
            assert!(e.contains("1 a 60"), "{e}");
        }
    }

    #[test]
    fn the_click_point_is_the_center_of_the_cell_in_screen_coordinates() {
        let g = tela(10, 6);
        assert_eq!(g.centro_na_tela(1).unwrap(), (96, 90));
        assert_eq!(g.centro_na_tela(60).unwrap(), (1824, 990));
    }

    #[test]
    fn dpi_scaling_and_a_monitor_offset_are_applied_to_the_click_point() {
        // Imagem em pixels físicos (3840x2160), monitor lógico 1920x1080 começando em (100, 50).
        let g = GradeAtual {
            imagem_w: 3840,
            imagem_h: 2160,
            monitor_x: 100,
            monitor_y: 50,
            monitor_w: 1920,
            monitor_h: 1080,
            regiao: Retangulo { x: 0.0, y: 0.0, w: 3840.0, h: 2160.0 },
            colunas: 10,
            linhas: 6,
        };
        // Célula 1: centro na imagem (192, 180) -> metade em coordenada lógica (96, 90) + deslocamento.
        assert_eq!(g.centro_na_tela(1).unwrap(), (196, 140));
    }

    #[test]
    fn refining_a_cell_makes_a_new_grid_inside_it() {
        let g = tela(10, 6);
        // Célula 12 = segunda coluna da segunda linha: x 192..384, y 180..360.
        let f = g.refinar(12, 6, 4).unwrap();
        assert_eq!((f.regiao.x, f.regiao.y, f.regiao.w, f.regiao.h), (192.0, 180.0, 192.0, 180.0));
        assert_eq!((f.colunas, f.linhas, f.total()), (6, 4, 24));
        // Célula 1 da grade fina: canto da região; seu centro está dentro da célula 12 da grossa.
        let (x, y) = f.centro_na_tela(1).unwrap();
        assert!((192..384).contains(&x) && (180..360).contains(&y), "({x},{y})");
        assert_eq!((x, y), (208, 203));
        // Refinar de novo desce mais um nível, e a grade original não muda.
        let ff = f.refinar(24, 2, 2).unwrap();
        assert!(ff.regiao.w < f.regiao.w && ff.regiao.x > f.regiao.x);
        assert_eq!(g.regiao.w, 1920.0);
        assert!(f.refinar(25, 2, 2).is_err());
    }

    #[test]
    fn the_crop_stays_inside_the_image() {
        let f = tela(10, 6).refinar(60, 6, 4).unwrap();
        let (x, y, w, h) = f.recorte();
        assert_eq!((x, y, w, h), (1728, 900, 192, 180));
        assert!(x + w <= 1920 && y + h <= 1080);
    }

    #[test]
    fn the_requested_size_is_limited_to_a_readable_range() {
        assert_eq!(dimensao(None, 10), 10);
        assert_eq!(dimensao(Some(-5), 10), 10);
        assert_eq!(dimensao(Some(0), 10), DIM_MIN);
        assert_eq!(dimensao(Some(1), 10), DIM_MIN);
        assert_eq!(dimensao(Some(8), 10), 8);
        assert_eq!(dimensao(Some(500), 10), DIM_MAX);
    }

    #[test]
    fn the_last_grid_is_kept_for_the_next_click_and_can_be_cleared() {
        guardar(tela(4, 3));
        assert_eq!(atual().unwrap().total(), 12);
        limpar();
        assert!(atual().is_none());
    }

    #[test]
    fn numbers_grow_with_the_cell_but_stay_between_1_and_4() {
        assert_eq!(escala_do_numero(10.0, 10.0), 1);
        assert_eq!(escala_do_numero(192.0, 180.0), 4);
        assert_eq!(escala_do_numero(60.0, 200.0), 2, "limita pela largura da célula");
        assert_eq!(escala_do_numero(1000.0, 1000.0), 4);
    }

    #[test]
    fn the_grid_draws_lines_and_a_label_in_every_cell_without_covering_the_rest() {
        let mut img = RgbaImage::from_pixel(600, 400, Rgba([255, 255, 255, 255]));
        desenhar_grade(&mut img, 3, 2);
        assert_eq!((img.width(), img.height()), (600, 400));
        // Linha vertical em x=200 e horizontal em y=200: branco misturado com magenta (não branco puro).
        assert_ne!(img.get_pixel(200, 300).0, [255, 255, 255, 255]);
        assert_ne!(img.get_pixel(100, 200).0, [255, 255, 255, 255]);
        // O miolo da célula fica intacto (a grade não cobre o conteúdo).
        assert_eq!(img.get_pixel(100, 100).0, [255, 255, 255, 255]);
        assert_eq!(img.get_pixel(500, 350).0, [255, 255, 255, 255]);
        // O canto de cada célula tem a tarjeta escura do número: 6 células, 6 cantos escurecidos.
        for (cx, cy) in [(0, 0), (200, 0), (400, 0), (0, 200), (200, 200), (400, 200)] {
            let p = img.get_pixel(cx + 3, cy + 3).0;
            assert!(p[0] < 120 && p[1] < 120, "canto ({cx},{cy}) = {p:?}");
        }
        // Há pixels amarelos (o número) na tela.
        let amarelos = img.pixels().filter(|p| p.0 == [255, 235, 0, 255]).count();
        assert!(amarelos > 6 * 5, "{amarelos}");
    }

    #[test]
    fn two_digit_numbers_use_more_room_than_one_digit() {
        let mut a = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        let mut b = a.clone();
        desenhar_numero(&mut a, 0, 0, 7, 2);
        desenhar_numero(&mut b, 0, 0, 77, 2);
        let escuros = |i: &RgbaImage| i.pixels().filter(|p| p.0[0] < 200).count();
        assert!(escuros(&b) > escuros(&a));
    }

    #[test]
    fn drawing_near_the_edges_never_panics() {
        let mut img = RgbaImage::from_pixel(5, 5, Rgba([10, 10, 10, 255]));
        desenhar_grade(&mut img, 20, 20);
        desenhar_numero(&mut img, 3, 3, 400, 4);
    }
}
