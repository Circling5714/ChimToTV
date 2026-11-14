//! Mạng: proxy SOCKS5 / Shadowsocks (thử kết nối, đọc link ss://), DNS mã hoá, vượt chặn — bảng hàm cho NetCore.cs của app WPF.
//! Lỗi phía proxy trả kèm dấu «socks|» / «ss|» để app báo «Lỗi proxy: …» / «Lỗi Shadowsocks: …» như trước.

use crate::Res;
use cbtv::net::SocksConfig;
use cbtv::shadowsocks::{self, SsConfig, SsMethod};
use cbtv::util::Error;
use cbtv::{doh, net, unblock};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::net::IpAddr;

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn b(a: &Value, k: &str) -> bool {
    a.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn port(a: &Value, k: &str) -> u16 {
    a.get(k).and_then(|v| v.as_u64()).unwrap_or(0).min(65535) as u16
}

fn set(a: &Value, k: &str) -> HashSet<String> {
    a.get(k)
        .and_then(|v| v.as_array())
        .map(|x| x.iter().filter_map(|v| v.as_str()).map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

/// Lỗi phía proxy → «<dấu>|câu báo» (Socks(_, m) chỉ lấy câu báo, không thêm «Lỗi proxy:» — app tự thêm).
fn proxy_err(tag: &str, e: Error) -> String {
    match e {
        Error::Socks(_, m) => format!("{tag}|{m}"),
        e => cbtv::util::friendly(&e),
    }
}

fn ss_config(a: &Value) -> SsConfig {
    let mut c = SsConfig::new(&s(a, "host"), port(a, "port"), &s(a, "method"), &s(a, "password"));
    c.remote_dns = a.get("remoteDns").and_then(|v| v.as_bool()).unwrap_or(true);
    c.sites_only = b(a, "sitesOnly");
    c
}

fn ss_json(c: &SsConfig) -> Value {
    json!({ "host": c.host, "port": c.port, "method": c.method, "password": c.password, "remoteDns": c.remote_dns, "sitesOnly": c.sites_only })
}

pub fn sync(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        "net.note" => Ok(json!(net::note())),
        "doh.providers" => Ok(json!(doh::providers())),
        "doh.servers" => Ok(json!(doh::servers().iter().map(|x| x.label.clone()).collect::<Vec<_>>())),
        "doh.debugFail" => Ok(json!(doh::debug_fail().into_iter().collect::<Vec<_>>())),
        "doh.setDebugFail" => {
            // nhãn máy chủ giữ nguyên chữ hoa / thường (DoH NextDNS…)
            let labels: HashSet<String> = a
                .get("labels")
                .and_then(|v| v.as_array())
                .map(|x| x.iter().filter_map(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default();
            doh::set_debug_fail(labels);
            Ok(Value::Null)
        }
        "doh.setDebugDnsBlock" => {
            let ip: IpAddr = s(a, "ip").parse().unwrap_or(IpAddr::from([0, 0, 0, 0]));
            doh::set_debug_dns_block(set(a, "hosts"), ip);
            Ok(Value::Null)
        }
        "doh.debugDnsBlock" => {
            let (h, ip) = doh::debug_dns_block();
            Ok(json!({ "hosts": h.into_iter().collect::<Vec<_>>(), "ip": ip.to_string() }))
        }
        "doh.forgetAnswers" => {
            doh::forget_answers();
            Ok(Value::Null)
        }
        "unblock.setDebugWholeHello" => {
            unblock::set_debug_whole_hello(set(a, "hosts"));
            Ok(Value::Null)
        }
        "unblock.debugWholeHello" => Ok(json!(unblock::debug_whole_hello().into_iter().collect::<Vec<_>>())),
        "unblock.level2" => Ok(json!({ "hosts": unblock::level2_memo::all(), "autoLevel2": unblock::level2_memo::auto_count() })),
        "unblock.level2Clear" => {
            unblock::level2_memo::clear();
            Ok(Value::Null)
        }
        // Khoá / mật khẩu Shadowsocks đúng dạng chưa (ô nhập trong Cài đặt → Proxy)
        "ss.keys" => match SsMethod::of(&s(a, "method")) {
            None => Err(format!("ss|Không hỗ trợ kiểu mã hoá {}", s(a, "method"))),
            Some(meth) => shadowsocks::keys(&meth, &s(a, "password")).map(|_| Value::Null).map_err(|e| proxy_err("ss", e)),
        },
        "ss.parseLink" => shadowsocks::parse_link(&s(a, "link"), a.get("remoteDns").and_then(|v| v.as_bool()).unwrap_or(true), b(a, "sitesOnly"))
            .map(|c| ss_json(&c))
            .map_err(|e| proxy_err("ss", e)),
        "ss.splitHostPort" => Ok(shadowsocks::split_host_port(&s(a, "s")).map(|(h, p)| json!({ "host": h, "port": p })).unwrap_or(Value::Null)),
        _ => return None,
    })
}

pub async fn call(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        // «Thử kết nối» Shadowsocks: IP ra ngoài, vd "103.1.2.3 (SG)"
        "ss.check" => shadowsocks::check(&ss_config(a)).await.map(|ip| json!(ip)).map_err(|e| proxy_err("ss", e)),
        // «Thử kết nối» SOCKS5: mở kết nối tới trang qua proxy (theo ô đang nhập, chưa lưu)
        "net.testSocks" => {
            let c = SocksConfig {
                host: s(a, "host"),
                port: port(a, "port"),
                user: s(a, "user"),
                pass: s(a, "pass"),
                remote_dns: a.get("remoteDns").and_then(|v| v.as_bool()).unwrap_or(true),
                sites_only: false,
            };
            let t0 = std::time::Instant::now();
            match tokio::time::timeout(std::time::Duration::from_secs(20), net::socks5_connect(&c, &s(a, "target"), port(a, "targetPort").max(1))).await {
                Ok(Ok(_)) => Ok(json!(t0.elapsed().as_millis() as u64)),
                Ok(Err(e)) => Err(proxy_err("socks", e)),
                Err(_) => Err(cbtv::util::friendly(&Error::Timeout)),
            }
        }
        _ => return None,
    })
}
