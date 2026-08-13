use k256::{
    AffinePoint, ProjectivePoint, Scalar, EncodedPoint, FieldBytes,
    elliptic_curve::{PrimeField, sec1::{FromEncodedPoint, ToEncodedPoint}},
};

use core::result::Result;

/// ElGamal 加密：
/// 输入公钥字节、消息 hash、可选随机数 r
/// 输出 (C1, C2)
pub fn elgamal_encrypt_secp256k1(
    pk_bytes: &[u8],
    msg: &[u8; 32],
    r_opt: Option<&[u8; 32]>,
) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    // 解析公钥
    let ep = EncodedPoint::from_bytes(pk_bytes).map_err(|_| "invalid pk encoding")?;

    let pk_affine = AffinePoint::from_encoded_point(&ep)
    .into_option()
    .ok_or("invalid public key point")?;
    let fb_msg = FieldBytes::from_slice(msg);
    let m_scalar = Scalar::from_repr(*fb_msg).unwrap_or(Scalar::ONE);

    // 生成临时随机标量 r
    let r_scalar = if let Some(rb) = r_opt {
        let fb_r = FieldBytes::from_slice(rb);
        Scalar::from_repr(*fb_r).unwrap_or(Scalar::ONE)
    } else {
        let fb_r = FieldBytes::from_slice(msg);
        Scalar::from_repr(*fb_r).unwrap_or(Scalar::ONE)
    };

    // 计算
    // C1 = rG
    let g_aff = AffinePoint::GENERATOR;
    let m_proj = ProjectivePoint::from(g_aff) * m_scalar;
    let c1_proj = ProjectivePoint::GENERATOR * r_scalar;
    // C2 = M + rPk
    let rpk_proj = ProjectivePoint::from(pk_affine) * r_scalar;
    let c2_proj = m_proj + rpk_proj;

    // 转为压缩字节
    let c1_bytes = c1_proj.to_affine().to_encoded_point(true).as_bytes().to_vec();
    let c2_bytes = c2_proj.to_affine().to_encoded_point(true).as_bytes().to_vec();

    Ok((c1_bytes, c2_bytes))
}
