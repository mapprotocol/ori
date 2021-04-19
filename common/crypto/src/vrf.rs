// Copyright 2021 MAP Protocol Authors.
// This file is part of MAP Protocol.

// MAP Protocol is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// MAP Protocol is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with MAP Protocol.  If not, see <http://www.gnu.org/licenses/>.

use std::borrow::Borrow;
use std::fmt;
use std::mem::transmute;

use blake2b_rs::Blake2bBuilder;
use blake2::Blake2b as Hash512;
use blake2::VarBlake2b;
use arrayref::{array_ref, array_refs, mut_array_refs};
use digest::generic_array::{typenum::U32, GenericArray};
use digest::{BlockInput, FixedOutput, Input, Reset, VariableOutput};
use ed25519_dalek;
use curve25519_dalek::traits::VartimeMultiscalarMul;
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint as Point};
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::constants::{
    RISTRETTO_BASEPOINT_POINT as G,
    RISTRETTO_BASEPOINT_TABLE as GT,
};
use rand_core::OsRng;
use subtle::{ConditionallySelectable, ConstantTimeEq};

#[derive(Clone)]
pub struct Hash256(VarBlake2b);

impl Default for Hash256 {
    fn default() -> Self {
        Hash256(VarBlake2b::new(32).unwrap())
    }
}

impl Input for Hash256 {
    fn input<B: AsRef<[u8]>>(&mut self, data: B) {
        self.0.input(data);
    }
}

impl BlockInput for Hash256 {
    type BlockSize = <VarBlake2b as BlockInput>::BlockSize;
}

impl FixedOutput for Hash256 {
    type OutputSize = U32;

    fn fixed_result(self) -> GenericArray<u8, U32> {
        let mut r = [0; 32];
        self.0.variable_result(|s| {
            r = *array_ref!(s, 0, 32);
        });
        r.into()
    }
}

impl Reset for Hash256 {
    fn reset(&mut self) {
        self.0.reset();
    }
}

mod hashable_trait {
    pub trait Hashable {
        fn hash_into<D: super::Input>(self, digest: D) -> D;
    }
}

use hashable_trait::*;

impl<T: AsRef<[u8]> + ?Sized> Hashable for &T {
    fn hash_into<D: Input>(self, digest: D) -> D {
        digest.chain(self.as_ref())
    }
}

impl Hashable for Point {
    fn hash_into<D: Input>(self, digest: D) -> D {
        digest.chain(&self.pack())
    }
}

impl Hashable for Scalar {
    fn hash_into<D: Input>(self, digest: D) -> D {
        digest.chain(&self.pack())
    }
}

pub fn _hash_new<D: Default>() -> D {
    D::default()
}

pub fn _hash_chain<D: Input, T: Hashable>(digest: D, data: T) -> D {
    data.hash_into(digest)
}

pub fn _hash_result<D: FixedOutput<OutputSize = U32>>(digest: D) -> [u8; 32] {
    digest.fixed_result().into()
}

pub fn _hash_to_scalar(hash: [u8; 32]) -> Scalar {
    Scalar::from_bytes_mod_order(hash)
}

macro_rules! hash_chain {
    ($h:expr, $d:expr $(, $dd:expr)*) => {
        hash_chain!(_hash_chain($h, $d) $(, $dd)*)
    };
    ($h:expr) => {
        $h
    };
}

macro_rules! hash {
    ($($d:expr),*) => {
        _hash_result(hash_chain!(_hash_new::<Hash256>() $(, $d)*))
    };
}

macro_rules! hash_s {
    ($($d:expr),*) => {
        _hash_to_scalar(hash!($($d),*))
    };
}

fn _prs_result(digest: Hash512) -> Scalar {
    let res = digest.fixed_result();
    Scalar::from_bytes_mod_order_wide(array_ref!(res, 0, 64))
}

macro_rules! prs {
    ($($d:expr),*) => {
        _prs_result(hash_chain!(_hash_new::<Hash512>() $(, $d)*))
    };
}

macro_rules! eq {
    ($ty:ty, $e:expr) => {
        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                ::std::convert::identity::<fn(&Self, &Self) -> bool>($e)(self, other)
            }
        }

        impl Eq for $ty {}
    };
}

macro_rules! unwrap_or_return_false {
    ($e:expr) => {
        match $e {
            ::std::option::Option::Some(v) => v,
            ::std::option::Option::None => return false,
        }
    };
}

macro_rules! bytes_type {
    ($vis:vis, $ty:ident, $l:literal, $what:literal) => {
        #[derive(Copy, Clone)]
        $vis struct $ty(pub [u8; $l]);

        eq!($ty, |a, b| a.0[..] == b.0[..]);

        impl AsMut<[u8; $l]> for $ty {
            fn as_mut(&mut self) -> &mut [u8; $l] {
                &mut self.0
            }
        }

        impl AsMut<[u8]> for $ty {
            fn as_mut(&mut self) -> &mut [u8] {
                &mut self.0[..]
            }
        }

        impl From<&[u8; $l]> for $ty {
            fn from(value: &[u8; $l]) -> Self {
                Self(*value)
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                for i in self.0.iter() {
                    write!(f, "{:02x}", i)?;
                }
                Ok(())
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "0x")?;
                for i in self.0.iter() {
                    write!(f, "{:02x}", i)?;
                }
                Ok(())
            }
        }
    };
}

pub trait Packable: Sized {
    type Packed;
    fn unpack(data: &Self::Packed) -> Option<Self>;
    fn pack(&self) -> Self::Packed;
}

pub fn unpack<T: Packable>(data: &T::Packed) -> Option<T> {
    Packable::unpack(data)
}

impl Packable for [u8; 32] {
    type Packed = [u8; 32];

    fn unpack(data: &[u8; 32]) -> Option<Self> {
        Some(*data)
    }

    fn pack(&self) -> [u8; 32] {
        *self
    }
}

impl Packable for Point {
    type Packed = [u8; 32];

    fn unpack(data: &[u8; 32]) -> Option<Self> {
        CompressedRistretto(*data).decompress()
    }

    fn pack(&self) -> [u8; 32] {
        self.compress().to_bytes()
    }
}

impl Packable for Scalar {
    type Packed = [u8; 32];

    fn unpack(data: &[u8; 32]) -> Option<Self> {
        Scalar::from_canonical_bytes(*data)
    }

    fn pack(&self) -> [u8; 32] {
        self.to_bytes()
    }
}

impl<T1: Packable<Packed = [u8; 32]>, T2: Packable<Packed = [u8; 32]>> Packable for (T1, T2) {
    type Packed = [u8; 64];

    fn unpack(data: &[u8; 64]) -> Option<Self> {
        let (d1, d2) = array_refs!(data, 32, 32);
        Some((unpack(d1)?, unpack(d2)?))
    }

    fn pack(&self) -> [u8; 64] {
        let mut res = [0; 64];
        let (d1, d2) = mut_array_refs!(&mut res, 32, 32);
        *d1 = self.0.pack();
        *d2 = self.1.pack();
        res
    }
}

impl<
        T1: Packable<Packed = [u8; 32]>,
        T2: Packable<Packed = [u8; 32]>,
        T3: Packable<Packed = [u8; 32]>,
    > Packable for (T1, T2, T3)
{
    type Packed = [u8; 96];

    fn unpack(data: &[u8; 96]) -> Option<Self> {
        let (d1, d2, d3) = array_refs!(data, 32, 32, 32);
        Some((unpack(d1)?, unpack(d2)?, unpack(d3)?))
    }

    fn pack(&self) -> [u8; 96] {
        let mut res = [0; 96];
        let (d1, d2, d3) = mut_array_refs!(&mut res, 32, 32, 32);
        *d1 = self.0.pack();
        *d2 = self.1.pack();
        *d3 = self.2.pack();
        res
    }
}

fn basemul(s: Scalar) -> Point {
    &s * &GT
}

fn safe_invert(s: Scalar) -> Scalar {
    Scalar::conditional_select(&s, &Scalar::one(), s.ct_eq(&Scalar::zero())).invert()
}

fn vmul2(s1: Scalar, p1: &Point, s2: Scalar, p2: &Point) -> Point {
    Point::vartime_multiscalar_mul(&[s1, s2], [p1, p2].iter().copied())
}

#[allow(dead_code)]
fn hash_to_scalar(pk: &[u8], input: &[u8]) -> Scalar {
    let mut hash_256 = Blake2bBuilder::new(32).build();
    let mut hash: [u8; 32] = [0; 32];

    let mut pre = Vec::new();
    pre.extend_from_slice(pk);
    pre.extend_from_slice(input);
    hash_256.update(&pre);
    hash_256.finalize(&mut hash);

    Scalar::from_bytes_mod_order(hash)
}

// Output of VRF function
bytes_type!(pub, Value, 32, "value");
// Validation proof of VRF function
bytes_type!(pub, Proof, 64, "proof");

#[derive(Copy, Clone)]
pub struct PublicKey(pub(crate) [u8; 32], pub(crate) Point);

impl PublicKey {
    #[allow(dead_code)]
    fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        Some(PublicKey(*bytes, unpack(bytes)?))
    }

    fn offset(&self, input: &[u8]) -> Scalar {
        // hash_to_scalar(&self.0, input)
        hash_s!(&self.0, input)
    }

    pub fn is_vrf_valid(&self, input: &impl Borrow<[u8]>, value: &Value, proof: &Proof) -> bool {
        self.is_valid(input.borrow(), value, proof)
    }

    fn is_valid(&self, input: &[u8], value: &Value, proof: &Proof) -> bool {
        let p = unwrap_or_return_false!(unpack(&value.0));
        let (r, c) = unwrap_or_return_false!(unpack(&proof.0));
        hash_s!(
            &self.0,
            &value.0,
            vmul2(r + c * self.offset(input), &G, c, &self.1),
            vmul2(r, &p, c, &G)
        ) == c
    }
}

#[derive(Copy, Clone)]
pub struct SecretKey(pub(crate) Scalar, pub(crate) PublicKey);

impl SecretKey {
    pub(crate) fn from_scalar(sk: Scalar) -> Self {
        let pk = basemul(sk);
        SecretKey(sk, PublicKey(pk.pack(), pk))
    }

    #[allow(dead_code)]
    fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        Some(Self::from_scalar(unpack(bytes)?))
    }

    pub fn random() -> Self {
        Self::from_scalar(Scalar::random(&mut OsRng))
    }

    pub fn public_key(&self) -> PublicKey {
        self.1
    }

    pub fn compute_vrf(&self, input: &impl Borrow<[u8]>) -> Value {
        self.compute(input.borrow())
    }

    fn compute(&self, input: &[u8]) -> Value {
        Value(basemul(safe_invert(self.0 + self.1.offset(input))).pack())
    }

    pub fn compute_vrf_with_proof(&self, input: &impl Borrow<[u8]>) -> (Value, Proof) {
        self.compute_with_proof(input.borrow())
    }

    fn compute_with_proof(&self, input: &[u8]) -> (Value, Proof) {
        let x = self.0 + self.1.offset(input);
        let inv = safe_invert(x);
        let val = basemul(inv).pack();
        let k = prs!(x);
        let c = hash_s!(&(self.1).0, &val, basemul(k), basemul(inv * k));
        (Value(val), Proof((k - c * x, c).pack()))
    }

    pub fn is_vrf_valid(&self, input: &impl Borrow<[u8]>, value: &Value, proof: &Proof) -> bool {
        self.1.is_valid(input.borrow(), value, proof)
    }
}

pub fn convert_secret_key(key: &[u8]) -> SecretKey {
    let b = ed25519_dalek::ExpandedSecretKey::from(
        &ed25519_dalek::SecretKey::from_bytes(key).unwrap(),
    ).to_bytes();

    SecretKey::from_scalar(Scalar::from_bytes_mod_order(*array_ref!(&b, 0, 32)))
}

pub fn convert_public_key(key: &[u8]) -> Option<PublicKey> {
    let ep: EdwardsPoint = CompressedEdwardsY::from_slice(&key).decompress()?;

    if !ep.is_torsion_free() {
        return None;
    }
    let rp: Point = unsafe { transmute(ep) };
    Some(PublicKey(rp.compress().to_bytes(), rp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_proof() {
        let sk = SecretKey::random();
        let (val, proof) = sk.compute_vrf_with_proof(b"Test");
        assert!(sk.public_key().is_vrf_valid(b"Test", &val, &proof));
        assert!(!sk.public_key().is_vrf_valid(b"Tent", &val, &proof));
    }
}
