//! Direct bindings to the external ERFA C library. No project C++ is called.
#![allow(non_snake_case, non_camel_case_types)]
use std::ffi::{c_char, c_int};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EraAstrom {
    pub pmt: f64,
    pub eb: [f64; 3],
    pub eh: [f64; 3],
    pub em: f64,
    pub v: [f64; 3],
    pub bm1: f64,
    pub bpn: [[f64; 3]; 3],
    pub along: f64,
    pub phi: f64,
    pub xpl: f64,
    pub ypl: f64,
    pub sphi: f64,
    pub cphi: f64,
    pub diurab: f64,
    pub eral: f64,
    pub refa: f64,
    pub refb: f64,
}
pub type eraASTROM = EraAstrom;

#[link(name = "erfa")]
unsafe extern "C" {
    pub fn eraDat(year: c_int, month: c_int, day: c_int, fraction: f64, dat: *mut f64) -> c_int;
    pub fn eraDtdb(day: f64, fraction: f64, ut: f64, elong: f64, u: f64, v: f64) -> f64;
    pub fn eraDtf2d(
        scale: *const c_char,
        year: c_int,
        month: c_int,
        day: c_int,
        hour: c_int,
        minute: c_int,
        second: f64,
        d1: *mut f64,
        d2: *mut f64,
    ) -> c_int;
    pub fn eraD2dtf(
        scale: *const c_char,
        ndp: c_int,
        d1: f64,
        d2: f64,
        year: *mut c_int,
        month: *mut c_int,
        day: *mut c_int,
        ihmsf: *mut c_int,
    ) -> c_int;
    pub fn eraUtctai(utc1: f64, utc2: f64, tai1: *mut f64, tai2: *mut f64) -> c_int;
    pub fn eraTaiutc(tai1: f64, tai2: f64, utc1: *mut f64, utc2: *mut f64) -> c_int;
    pub fn eraTdbtcb(tdb1: f64, tdb2: f64, tcb1: *mut f64, tcb2: *mut f64) -> c_int;
    pub fn eraUtcut1(utc1: f64, utc2: f64, dut1: f64, ut11: *mut f64, ut12: *mut f64) -> c_int;
    pub fn eraLtpb(epoch: f64, matrix: *mut [f64; 3]);
    pub fn eraLtpequ(epoch: f64, pole: *mut f64);
    pub fn eraLtpecl(epoch: f64, pole: *mut f64);
    pub fn eraNut00a(day: f64, fraction: f64, dpsi: *mut f64, deps: *mut f64);
    pub fn eraNumat(eps: f64, dpsi: f64, deps: f64, matrix: *mut [f64; 3]);
    pub fn eraXys06a(day: f64, fraction: f64, x: *mut f64, y: *mut f64, s: *mut f64);
    pub fn eraC2ixys(x: f64, y: f64, s: f64, matrix: *mut [f64; 3]);
    pub fn eraEra00(day: f64, fraction: f64) -> f64;
    pub fn eraRz(angle: f64, matrix: *mut [f64; 3]);
    pub fn eraSp00(day: f64, fraction: f64) -> f64;
    pub fn eraPom00(xp: f64, yp: f64, sp: f64, matrix: *mut [f64; 3]);
    pub fn eraEpv00(
        day: f64,
        fraction: f64,
        heliocentric: *mut [f64; 3],
        barycentric: *mut [f64; 3],
    ) -> c_int;
    pub fn eraApco(
        day: f64,
        fraction: f64,
        ebpv: *mut [f64; 3],
        ehp: *mut f64,
        x: f64,
        y: f64,
        s: f64,
        theta: f64,
        elong: f64,
        phi: f64,
        hm: f64,
        xp: f64,
        yp: f64,
        sp: f64,
        refa: f64,
        refb: f64,
        astrom: *mut EraAstrom,
    );
    pub fn eraApcs(
        day: f64,
        fraction: f64,
        pv: *mut [f64; 3],
        ebpv: *mut [f64; 3],
        ehp: *mut f64,
        astrom: *mut EraAstrom,
    );
    pub fn eraApcs13(day: f64, fraction: f64, pv: *mut [f64; 3], astrom: *mut EraAstrom);
    pub fn eraAtciqz(rc: f64, dc: f64, astrom: *mut EraAstrom, ri: *mut f64, di: *mut f64);
    pub fn eraAtciq(
        rc: f64,
        dc: f64,
        pr: f64,
        pd: f64,
        px: f64,
        rv: f64,
        astrom: *mut EraAstrom,
        ri: *mut f64,
        di: *mut f64,
    );
    pub fn eraAtioq(
        ri: f64,
        di: f64,
        astrom: *mut EraAstrom,
        aob: *mut f64,
        zob: *mut f64,
        hob: *mut f64,
        dob: *mut f64,
        rob: *mut f64,
    );
    pub fn eraLdsun(p: *mut f64, e: *mut f64, em: f64, p1: *mut f64);
    pub fn eraLd(bm: f64, p: *mut f64, q: *mut f64, e: *mut f64, em: f64, dlim: f64, p1: *mut f64);
    pub fn eraAb(pnat: *mut f64, v: *mut f64, s: f64, bm1: f64, ppr: *mut f64);
    pub fn eraPmsafe(
        ra1: f64,
        dec1: f64,
        pmr1: f64,
        pmd1: f64,
        px1: f64,
        rv1: f64,
        ep1a: f64,
        ep1b: f64,
        ep2a: f64,
        ep2b: f64,
        ra2: *mut f64,
        dec2: *mut f64,
        pmr2: *mut f64,
        pmd2: *mut f64,
        px2: *mut f64,
        rv2: *mut f64,
    ) -> c_int;
    pub fn eraGd2gc(n: c_int, elong: f64, phi: f64, height: f64, xyz: *mut f64) -> c_int;
    pub fn eraIcrs2g(ra: f64, dec: f64, lon: *mut f64, lat: *mut f64);
}
