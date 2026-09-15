//! Tratamento de imagem compartilhado — hoje, o redimensionamento que evita
//! gastar token (e banda) à toa com imagem maior do que o modelo aproveita.
//!
//! **Por que existe.** Imagem custa token proporcional à **área** (ver
//! `context.rs`, fórmula área/750). Medido nas 76 imagens reais do histórico do
//! usuário (2026-09-15): um print de tela cheia **1920×1080 custa 2.765
//! tokens**; o mesmo print a 1280px custa **1.229 (−56%)** e continua legível.
//! O payload também despenca — um PNG de 2.384 KB vira ~50 KB de JPEG (o
//! base64 de 3.179 KB ia inteiro pro request).
//!
//! **O limite é UM só, de propósito.** O `computer_use` já redimensionava as
//! próprias screenshots a 1280px, mas o caminho do usuário (colar/anexar)
//! passava reto, sem toque nenhum. Se cada caminho tivesse o seu número, um
//! desfaria o trabalho do outro — por isso a constante mora aqui e os dois
//! importam daqui.

use anyhow::{anyhow, Result};
use base64::Engine as _;
use image::{DynamicImage, Rgba, RgbaImage};

/// Maior lado permitido, em pixels.
///
/// 1280 é o valor que o `computer_use` já usava em produção, e fica abaixo dos
/// 1568px que a Anthropic documenta como limite de imagem — a faixa que os
/// provedores tratam como segura. Pra ler texto de screenshot dá ~6-7px por
/// caractere, que é legível; a 1024 começa a embaçar fonte pequena.
pub const MAX_IMAGE_EDGE: u32 = 1280;

/// Qualidade do JPEG — a mesma que o `computer_use` já usava.
pub const JPEG_QUALITY: u8 = 80;

/// Redimensiona (só quando passa do limite) e recomprime em JPEG.
///
/// Nunca aumenta uma imagem pequena: recorte de 179×307 continua 179×307.
/// Devolve os bytes do JPEG.
///
/// `filter` é parâmetro porque os dois caminhos têm prioridades diferentes:
/// screenshot do agente mantém o `Triangle` que já usava (não mexer no que
/// funciona), e o caminho do usuário usa `Lanczos3`, que preserva melhor texto
/// pequeno — que é justamente o caso crítico lá.
pub fn encode_jpeg(
    img: DynamicImage,
    max_edge: u32,
    quality: u8,
    filter: image::imageops::FilterType,
) -> Result<Vec<u8>> {
    // Achata transparência sobre branco ANTES de virar JPEG: JPEG não tem
    // canal alfa, e um `to_rgb8` cru pintaria as áreas transparentes de preto
    // (feio num ícone ou logo colado). PNG de screenshot costuma ser opaco,
    // mas isso custa pouco e evita o caso ruim.
    let img = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let mut base: RgbaImage =
            RgbaImage::from_pixel(rgba.width(), rgba.height(), Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut base, &rgba, 0, 0);
        DynamicImage::ImageRgba8(base)
    } else {
        img
    };

    let (w, h) = (img.width(), img.height());
    let img = if w.max(h) > max_edge {
        let ratio = max_edge as f32 / w.max(h) as f32;
        let nw = ((w as f32 * ratio).round() as u32).max(1);
        let nh = ((h as f32 * ratio).round() as u32).max(1);
        img.resize_exact(nw, nh, filter)
    } else {
        img
    };

    let rgb = img.to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .encode(&rgb, rgb.width(), rgb.height(), image::ColorType::Rgb8.into())
        .map_err(|e| anyhow!("falha ao codificar JPEG: {e}"))?;
    Ok(buf.into_inner())
}

/// Otimiza um data URI (`data:image/png;base64,...`) devolvendo outro já
/// redimensionado e em JPEG. É o caminho de quem cola imagem na área de
/// transferência, onde os bytes só existem no navegador.
pub fn optimize_data_url(data_uri: &str) -> Result<String> {
    let b64 = data_uri
        .split_once(',')
        .map(|(_, payload)| payload)
        .ok_or_else(|| anyhow!("data URI sem o separador ',' esperado"))?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| anyhow!("base64 inválido: {e}"))?;

    let img = image::load_from_memory(&bytes).map_err(|e| anyhow!("imagem inválida: {e}"))?;

    let precisou_reduzir = img.width().max(img.height()) > MAX_IMAGE_EDGE;

    // Lanczos3 aqui: o caso crítico é screenshot de código com fonte pequena.
    let jpeg = encode_jpeg(
        img,
        MAX_IMAGE_EDGE,
        JPEG_QUALITY,
        image::imageops::FilterType::Lanczos3,
    )?;

    // Se NÃO precisou reduzir e o JPEG ficou maior que o original, fica com o
    // original. Motivo medido (2026-09-15): PNG de screenshot, com cores
    // chapadas, comprime **melhor** que JPEG q80 — vi caso de 185 KB de PNG
    // virando 199 KB. E não há ganho de token na troca: token depende só da
    // dimensão, que aqui não mudou.
    if !precisou_reduzir && jpeg.len() >= bytes.len() {
        return Ok(data_uri.to_string());
    }

    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&jpeg)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn imagem_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_pixel(w, h, Rgb([120, 140, 200]));
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn data_uri_png(w: u32, h: u32) -> String {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(imagem_png(w, h))
        )
    }

    fn dimensoes_do_data_uri(uri: &str) -> (u32, u32) {
        let b64 = uri.split_once(',').unwrap().1;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn imagem_grande_e_reduzida_ao_limite() {
        let otimizada = optimize_data_url(&data_uri_png(1920, 1080)).unwrap();
        let (w, h) = dimensoes_do_data_uri(&otimizada);
        assert_eq!(w, MAX_IMAGE_EDGE, "maior lado deveria bater no limite");
        assert_eq!(h, 720, "aspecto 16:9 preservado (1080 * 1280/1920)");
    }

    #[test]
    fn imagem_pequena_nao_e_aumentada() {
        // Regressão importante: recorte pequeno (o usuário tem um de 179x307)
        // não pode ser esticado — só aumentaria o custo e a imagem ficaria pior.
        let otimizada = optimize_data_url(&data_uri_png(179, 307)).unwrap();
        assert_eq!(dimensoes_do_data_uri(&otimizada), (179, 307));
    }

    #[test]
    fn imagem_exatamente_no_limite_fica_igual() {
        let otimizada = optimize_data_url(&data_uri_png(1280, 720)).unwrap();
        assert_eq!(dimensoes_do_data_uri(&otimizada), (1280, 720));
    }

    #[test]
    fn imagem_que_precisa_reduzir_sai_em_jpeg() {
        // Imagem maior que o limite passa pelo encoder e sai JPEG — é o que faz
        // o payload despencar. (Imagem que NÃO precisa reduzir pode continuar
        // PNG, se o JPEG ficar maior — ver
        // `nao_troca_por_jpeg_maior_quando_nao_precisa_reduzir`.)
        let otimizada = optimize_data_url(&data_uri_png(1920, 1080)).unwrap();
        assert!(
            otimizada.starts_with("data:image/jpeg;base64,"),
            "esperava JPEG, veio: {}",
            &otimizada[..40.min(otimizada.len())]
        );
    }

    #[test]
    fn imagem_com_alpha_vira_branco_nao_preto() {
        // PNG com transparência não pode virar fundo preto ao perder o alfa.
        // Usa imagem acima do limite pra forçar o caminho de encode (é lá que o
        // alfa é achatado; quando a imagem é mantida como veio, o alfa nem é
        // tocado).
        let mut rgba: RgbaImage = RgbaImage::from_pixel(1920, 1080, Rgba([0, 0, 0, 0]));
        rgba.put_pixel(5, 5, Rgba([255, 0, 0, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
        );

        let saida = optimize_data_url(&uri).unwrap();
        assert!(saida.starts_with("data:image/jpeg;base64,"));
        let b64 = saida.split_once(',').unwrap().1;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgb8();

        // Um canto (área transparente) tem que estar claro, não preto.
        let canto = img.get_pixel(0, 0);
        assert!(
            canto.0[0] > 200 && canto.0[1] > 200 && canto.0[2] > 200,
            "canto transparente deveria virar branco, veio {:?}",
            canto.0
        );
    }

    #[test]
    fn data_uri_invalido_da_erro_em_vez_de_panico() {
        assert!(optimize_data_url("nao-e-um-data-uri").is_err());
        assert!(optimize_data_url("data:image/png;base64,@@@nao-e-base64@@@").is_err());
    }

    #[test]
    fn payload_diminui_de_verdade() {
        // O ganho concreto: PNG grande -> JPEG pequeno.
        let uri = data_uri_png(1920, 1080);
        let antes = uri.len();
        let depois = optimize_data_url(&uri).unwrap().len();
        assert!(
            depois < antes,
            "payload deveria encolher: {antes} -> {depois}"
        );
    }

    #[test]
    fn nao_troca_por_jpeg_maior_quando_nao_precisa_reduzir() {
        // Regressão medida ao vivo (2026-09-15): PNG de screenshot, com cores
        // chapadas, ficou MAIOR em JPEG q80 (185 KB -> 199 KB). Sem redução de
        // dimensão não há ganho de token nenhum, então tem que ficar com o
        // original.
        //
        // Imagem de cor única é o caso extremo de "cores chapadas": o PNG fica
        // minúsculo e o JPEG não tem como competir.
        let uri = data_uri_png(200, 200);
        let saida = optimize_data_url(&uri).unwrap();
        assert!(
            saida.starts_with("data:image/png;base64,"),
            "sem precisar reduzir, deveria manter o PNG quando o JPEG não compensa"
        );
        assert!(saida.len() <= uri.len(), "não pode ficar maior que o original");
    }

    #[test]
    fn troca_por_jpeg_quando_o_png_era_grande_demais() {
        // O outro lado da regra: com redução de dimensão, a troca SEMPRE vale
        // (token cai, e o JPEG tende a ser menor que o PNG original).
        let uri = data_uri_png(1920, 1080);
        let saida = optimize_data_url(&uri).unwrap();
        assert!(saida.starts_with("data:image/jpeg;base64,"));
        assert!(saida.len() < uri.len());
    }
}
