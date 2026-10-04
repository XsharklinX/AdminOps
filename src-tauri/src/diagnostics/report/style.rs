//! El aspecto del informe: hoja de estilo, logo y tipografías.

use super::*;

pub(super) const CSS: &str = r#"
*{box-sizing:border-box}
html{background:#e8ecf1}
body{margin:0;color:#1b2330;font:12.5px/1.5 "Plex","IBM Plex Sans","Segoe UI",system-ui,sans-serif;counter-reset:sec;font-feature-settings:"tnum" 0}
main{max-width:840px;margin:24px auto;background:#fff;padding:44px 48px 36px;box-shadow:0 1px 4px rgba(20,30,50,.12)}
.top{display:flex;justify-content:space-between;gap:24px;align-items:flex-start}
.brand{display:flex;gap:14px;align-items:center;min-width:0}
.brand svg,.brand img{width:50px;height:50px;object-fit:contain;flex:none}
.brand .name{font-size:16px;font-weight:600;line-height:1.25}
.brand .contact{color:#5b6778;font-size:11px}
.doc{text-align:right;flex:none}
.doc .kind{font-size:21px;font-weight:600;letter-spacing:-.01em;line-height:1.2}
.doc .num{color:#5b6778;font-size:11.5px;margin-top:2px}.doc .num b{color:#1b2330;font-weight:600}
.rule{height:3px;background:#2f63d8;border-radius:2px;margin:18px 0 18px}
.info{display:grid;grid-template-columns:repeat(3,1fr);gap:10px}
.box{background:#f4f6f9;border-radius:8px;padding:10px 12px;min-width:0}
.box h4,.label{margin:0 0 4px;font-size:10px;font-weight:600;color:#5b6778;text-transform:uppercase;letter-spacing:.07em}
.box .main{font-weight:600;font-size:13px;overflow-wrap:anywhere}.box div{overflow-wrap:anywhere}.box .sub{color:#5b6778;font-size:11.5px}
.verdict{display:flex;gap:20px;align-items:center;border:1px solid #e3e8ef;border-left:5px solid var(--c);border-radius:10px;padding:14px 18px;margin-top:16px;break-inside:avoid}
.verdict .state{flex:1;min-width:0}.verdict .state .t{font-size:17px;font-weight:600;color:var(--c)}.verdict .state p{margin:3px 0 0;color:#3b4656}
.kpis{display:flex;gap:6px}.kpi{text-align:center;min-width:74px;padding:4px 6px}.kpi b{display:block;font-size:21px;font-weight:600;line-height:1.2;font-variant-numeric:tabular-nums}.kpi span{font-size:10.5px;color:#5b6778}
.v-ok{--c:#12784a}.v-warn{--c:#b06f00}.v-bad{--c:#c0223f}
h2{font-size:14px;font-weight:600;margin:26px 0 9px;padding-bottom:5px;border-bottom:1px solid #e3e8ef;break-after:avoid}
h2::before{counter-increment:sec;content:counter(sec) ". ";color:#2f63d8}
h3{font-size:12.5px;font-weight:600;margin:16px 0 6px;color:#3b4656;break-after:avoid}
p{margin:6px 0}
table{width:100%;border-collapse:collapse;font-size:12px}
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #edf0f4;vertical-align:top}
thead th,tr.head th{color:#5b6778;font-weight:600;font-size:11px;background:#f4f6f9;border-bottom:0}
table.kv th{width:30%;color:#5b6778;font-weight:500}
.nowrap{white-space:nowrap}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.muted{color:#8a95a5}.better{color:#12784a;font-weight:600}.worse{color:#c0223f;font-weight:600}.warn{color:#b06f00;font-weight:600}
.items{list-style:none;padding:0;margin:0;columns:2;column-gap:24px}
.items li{padding:2px 0 2px 20px;position:relative;break-inside:avoid}
.items li::before{position:absolute;left:0;top:2px;font-weight:700}
.items .y::before{content:"✓";color:#12784a}.items .n::before{content:"–";color:#8a95a5}.items .x::before{content:"!";color:#c0223f}
.items.one{columns:1}
.f{display:flex;gap:10px;padding:6px 10px;border-radius:6px;margin-bottom:5px;background:#f7f9fb;break-inside:avoid}
.f .dot{width:8px;height:8px;border-radius:50%;margin-top:6px;flex:none}
.dot.bad{background:#e0284a}.dot.warn{background:#e39a00}.dot.info{background:#2a8fc4}.dot.ok{background:#1f9d62}
.fgrid{display:grid;grid-template-columns:1fr 1fr;gap:0 8px}
.f b{font-weight:600}.f small{display:block;color:#5b6778}
.cols{display:grid;grid-template-columns:1fr 1fr;gap:20px}
.text{white-space:pre-wrap;background:#f7f9fb;border-left:3px solid #2f63d8;padding:10px 14px;border-radius:4px}
.bill thead th{padding:7px 8px}.bill td{padding:7px 8px}
.totals{width:300px;margin:8px 0 0 auto}.totals td{border:0;padding:3px 8px}
.totals tr.grand td{font-size:15px;font-weight:700;border-top:2px solid #1b2330;padding-top:7px}
.note{font-size:11px;color:#5b6778;margin-top:6px}
.intro{white-space:pre-wrap;margin:4px 0 14px;color:#2b3544}
.warranty td.num{width:120px}
.speed{display:flex;gap:10px}.speed div{flex:1;background:#f4f6f9;border-radius:8px;padding:8px 12px;font-size:11px;color:#5b6778}.speed b{display:block;font-size:17px;color:#1b2330;font-weight:600}
.sign{display:grid;grid-template-columns:1fr 1fr;gap:36px;margin-top:34px;break-inside:avoid}
.sign .pad{height:74px;display:flex;align-items:flex-end;justify-content:center;border-bottom:1px solid #1b2330;padding-bottom:3px}
.sign img{max-height:70px;max-width:100%}
.sign .who{text-align:center;margin-top:5px;font-size:11.5px}.sign .who span{display:block;color:#5b6778;font-size:10.5px}
.conditions{font-size:10px;color:#5b6778;white-space:pre-wrap;border-top:1px solid #e3e8ef;margin-top:26px;padding-top:8px}
footer{margin-top:22px;color:#8a95a5;font-size:10px;text-align:center}
.areas{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}
.area{border:1px solid #e3e8ef;border-left:4px solid var(--c);border-radius:8px;padding:8px 11px;break-inside:avoid;min-width:0}
.area .n{display:flex;justify-content:space-between;gap:8px;font-size:10.5px;font-weight:600;color:#5b6778;text-transform:uppercase;letter-spacing:.06em}
.area .s{color:var(--c);text-transform:none;letter-spacing:0}
.area p{margin:3px 0 0;font-size:11.5px;color:#2b3544}
.a-ok{--c:#12784a}.a-warn{--c:#b06f00}.a-bad{--c:#c0223f}.a-na{--c:#aab3c0}
.prio{display:inline-block;font-size:9.5px;font-weight:600;text-transform:uppercase;letter-spacing:.06em;border-radius:4px;padding:1px 6px;margin-right:6px;vertical-align:1px}
.prio.bad{background:#fbe6ea;color:#c0223f}.prio.warn{background:#fdf1dc;color:#935d00}
.ring{display:flex;flex-direction:column;align-items:center;min-width:74px}.ring svg{width:52px;height:52px}.ring span{font-size:10.5px;color:#5b6778}
.about{font-size:10px;color:#5b6778;border-top:1px solid #e3e8ef;margin-top:22px;padding-top:8px}
@media print{
*{-webkit-print-color-adjust:exact;print-color-adjust:exact}
html{background:#fff}main{margin:0;padding:0;max-width:none;box-shadow:none}
tr,.f,.box,.kpi{break-inside:avoid}footer{display:none}
}
"#;

pub(super) const LOGO: &str = include_str!("../../../../src/assets/logo.svg");

/// IBM Plex Sans dentro del propio informe: se ve igual en cualquier equipo,
/// tenga o no la fuente instalada (Edge caía a Segoe UI). Licencia OFL, en
/// assets/fonts.
pub(super) const FONTS: [(u16, &[u8]); 4] = [
    (400, include_bytes!("../../../assets/fonts/ibm-plex-sans-latin-400-normal.woff2")),
    (500, include_bytes!("../../../assets/fonts/ibm-plex-sans-latin-500-normal.woff2")),
    (600, include_bytes!("../../../assets/fonts/ibm-plex-sans-latin-600-normal.woff2")),
    (700, include_bytes!("../../../assets/fonts/ibm-plex-sans-latin-700-normal.woff2")),
];

pub(super) fn font_faces() -> String {
    FONTS
        .iter()
        .map(|(w, bytes)| format!("@font-face{{font-family:\"Plex\";src:url(data:font/woff2;base64,{})format(\"woff2\");font-weight:{w};font-style:normal}}", b64(bytes)))
        .collect()
}
