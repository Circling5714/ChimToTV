//! Cài đặt: trang chủ, hồ sơ trẻ em, tua, tự chuyển tập, server dự phòng, cỡ giao diện, toàn màn hình, DNS mã hoá, vượt chặn,
//! proxy, cập nhật, xoá lịch sử / dữ liệu duyệt web, phím tắt, giới thiệu. Bản Windows: SettingsPages.cs SettingsPage.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::px;
use crate::ui;
use cbtv::prefs;
use gtk::prelude::*;
use std::rc::Rc;

pub struct SettingsPage {
    base: PageBase,
}

pub fn new() -> Rc<dyn Page> {
    Rc::new(SettingsPage { base: PageBase::new("settings") })
}

type Choice = (String, bool, Rc<dyn Fn()>);

fn choice(text: impl Into<String>, active: bool, f: impl Fn() + 'static) -> Choice {
    (text.into(), active, Rc::new(f))
}

impl SettingsPage {
    fn option(self: &Rc<Self>, label: &str, choices: Vec<Choice>, note: Option<&str>) {
        let t = ui::label(label, &["h2"], true);
        ui::margins(&t, 14.0, 0.0, 8.0, 0.0);
        self.base.add(&t);
        let row = ui::flow(10.0, 10.0);
        for (text, active, act) in choices {
            let me = self.clone();
            let t2 = text.clone();
            let p = ui::pill_tag(&text, active.then_some("active"), move || {
                act();
                me.build();
                ui::refocus_label(me.base.body.upcast_ref(), Some(t2.clone()));
            });
            p.set_size_request(px(90.0), -1);
            ui::add_flow(&row, p.upcast_ref());
        }
        self.base.add(&row);
        if let Some(n) = note {
            let l = ui::dim(n);
            l.set_margin_bottom(px(4.0));
            self.base.add(&l);
        }
    }

    fn build(self: &Rc<Self>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Cài đặt"));
        let base = prefs::base_url();
        b.add(&ui::row(&[
            ui::pill(&format!("🌐  Trang phim: {}", crate::menu::site_display(&base)), || app().show_root(super::sites::new())).upcast(),
            ui::pill(&format!("👤  Người xem: {} — quản lý / giới hạn giờ xem…", prefs::current_profile().name), || {
                app().push(super::profiles_page::new())
            })
            .upcast(),
        ]));

        // Trang chủ (riêng từng người xem): tổng hợp / theo trang; hồ sơ trẻ em (tắt cần mã PIN phụ huynh)
        let who = if prefs::profiles().len() >= 2 { format!(" của {}", prefs::current_profile().name) } else { String::new() };
        let kids = prefs::kids();
        let mix = prefs::home_mix();
        self.option(
            &format!("Trang chủ{who}"),
            if kids {
                vec![choice("Tổng hợp (hồ sơ trẻ em)", true, || {})]
            } else {
                vec![choice("Tổng hợp mọi nguồn", mix, || prefs::set_home_mix(true)), choice("Theo trang đang dùng", !mix, || prefs::set_home_mix(false))]
            },
            Some(
                "Tổng hợp: trang chủ lấy phim mới từ mọi trang trong danh sách (KKPhim, NguonC, VSMOV, CLB Phim Xưa và trang đang dùng), \
                 gộp phim trùng giữa các trang («3 nguồn» — trang phim có nút đổi nguồn), xếp hàng theo lịch sử xem của từng người: \
                 phim đang xem dở có tập mới ở bất kỳ trang nào, phim giống phim vừa xem, thể loại hay xem. Trang nào lỗi / quá 6 giây \
                 thì dùng bản lưu; lỗi 3 lần liên tiếp thì tạm nghỉ 30 phút.",
            ),
        );
        let t = ui::label(&format!("Hồ sơ trẻ em{who}"), &["h2"], true);
        ui::margins(&t, 14.0, 0.0, 8.0, 0.0);
        b.add(&t);
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill_tag("Bật", kids.then_some("active"), move || {
                prefs::set_kids(true);
                m1.build();
            })
            .upcast(),
            ui::pill_tag("Tắt", (!kids).then_some("active"), move || {
                let m2 = m2.clone();
                ui::local(async move {
                    if !prefs::kids() {
                        return;
                    }
                    // Trẻ không tự tắt được: phải nhập mã PIN phụ huynh (nếu đã đặt)
                    if prefs::has_parent_pin() && !app().check_parent("Nhập mã PIN phụ huynh để tắt hồ sơ trẻ em.").await {
                        return;
                    }
                    prefs::set_kids(false);
                    m2.build();
                });
            })
            .upcast(),
        ]));
        b.add(&ui::dim(
            "Trang chủ chỉ có phim hoạt hình hợp trẻ em — thiếu nhi, gia đình, lồng tiếng; bỏ phim kinh dị / hình sự / chiến tranh / 18+. \
             Chỉ lọc trang chủ (tìm kiếm, mục của trang vẫn như cũ).",
        ));
        let rows = ui::pill(&format!("☰  Các hàng trên trang chủ{who}…"), || {
            ui::local(async {
                if prefs::kids() && prefs::has_parent_pin() && !app().check_parent("Nhập mã PIN phụ huynh để sửa các hàng trang chủ.").await {
                    return;
                }
                app().push(crate::hooks::home_rows_page());
            });
        });
        let sources = ui::pill(&crate::hooks::sources_label(), || {
            ui::local(async {
                if prefs::kids() && prefs::has_parent_pin() && !app().check_parent("Nhập mã PIN phụ huynh để sửa trang chủ.").await {
                    return;
                }
                app().push(crate::hooks::home_sources_page());
            });
        });
        let hr = ui::row(&[rows.upcast(), sources.upcast()]);
        ui::margins(&hr, 10.0, 0.0, 4.0, 0.0);
        b.add(&hr);
        b.add(&ui::dim("Trên trang chủ: nhấn phải (hoặc phím Menu / M) trên một phim của hàng để ẩn / chuyển nhanh hàng đó."));
        let resting = cbtv::feed::resting_hosts();
        if !resting.is_empty() {
            let until = cbtv::util::local_hm(*resting.values().max().unwrap_or(&0));
            let names: Vec<String> = resting.keys().map(|h| cbtv::site::tag(h, &prefs::sites())).collect();
            let me = self.clone();
            b.add(&ui::row(&[ui::pill(&format!("↻  Thử lại ngay: {} (đang tạm nghỉ tới {until})", names.join(", ")), move || {
                cbtv::feed::clear_rest();
                app().toast("Trang chủ sẽ hỏi lại các trang này ở lần tải tới.");
                me.build();
            })
            .upcast()]));
        }

        let step = prefs::seek_step();
        self.option(
            "Mỗi lần bấm ◀ ▶ khi xem phim tua",
            [5, 10, 15, 30].into_iter().map(|s| choice(format!("{s} giây"), step == s, move || prefs::set_seek_step(s))).collect(),
            Some("Giữ phím thì tua nhanh dần."),
        );
        let an = prefs::auto_next();
        self.option(
            "Hết tập tự chuyển sang tập sau",
            vec![choice("Bật", an, || prefs::set_auto_next(true)), choice("Tắt", !an, || prefs::set_auto_next(false))],
            None,
        );
        let kk = prefs::kk_backup();
        self.option(
            "Server dự phòng KKPhim",
            vec![choice("Bật", kk, || prefs::set_kk_backup(true)), choice("Tắt", !kk, || prefs::set_kk_backup(false))],
            Some(
                "Với yanhh3d, hoathinh3d, hhkungfu… (trình phát của trang hay hỏng cùng lúc) app tìm cùng phim, cùng tập trên KKPhim và \
                 thêm server «KKPhim Vietsub / Thuyết Minh» vào bảng ▼ lúc xem; trang không vào được mà KKPhim có tập đó thì vẫn xem được.",
            ),
        );
        let nc = prefs::nc_backup();
        self.option(
            "Server dự phòng NguonC",
            vec![choice("Bật", nc, || prefs::set_nc_backup(true)), choice("Tắt", !nc, || prefs::set_nc_backup(false))],
            Some("Như KKPhim nhưng tìm trên NguonC (hay có tập mới sớm) và thêm server «NguonC Vietsub…». Trình phát của NguonC chạy quảng cáo trước phim — app tự bỏ qua."),
        );
        let sc = prefs::ui_scale();
        self.option(
            "Cỡ giao diện",
            [1.0, 1.25, 1.5, 1.75]
                .into_iter()
                .map(|s| {
                    choice(format!("{}%", (s * 100.0) as i64), (sc - s).abs() < 0.01, move || {
                        prefs::set_ui_scale(s);
                        crate::css::install();
                        app().rebuild_menu();
                    })
                })
                .collect(),
            Some("Máy cắm tivi, ngồi xa: chọn 150% hoặc 175%."),
        );
        let fs = prefs::start_fullscreen();
        self.option(
            "Mở app ở chế độ toàn màn hình",
            vec![
                choice("Bật", fs, || {
                    prefs::set_start_fullscreen(true);
                    app().set_fullscreen(true);
                }),
                choice("Tắt", !fs, || {
                    prefs::set_start_fullscreen(false);
                    app().set_fullscreen(false);
                }),
            ],
            Some("F11 (hoặc Alt+Enter) để bật / tắt toàn màn hình bất cứ lúc nào; đang toàn màn hình thì menu có mục «Thoát toàn màn hình», Esc ở menu cũng thoát."),
        );
        let dns = prefs::secure_dns();
        let doh_note = format!(
            "Hỏi địa chỉ trang qua DNS mã hoá thay cho DNS nhà mạng: DoH rồi DoT, lần lượt {}; lỗi hết thì dùng DNS của máy — app không \
             hỏng khi DNS mã hoá bị chặn. Nhà mạng không đọc / sửa được truy vấn DNS (vẫn thấy tên trang lúc kết nối — Vượt chặn lo phần này).",
            crate::hooks::doh_providers()
        );
        self.option(
            "DNS mã hoá (DoH / DoT)",
            vec![
                choice("Bật", dns, || {
                    prefs::set_secure_dns(true);
                    cbtv::net::settings_changed();
                }),
                choice("Tắt", !dns, || {
                    prefs::set_secure_dns(false);
                    cbtv::net::settings_changed();
                }),
            ],
            Some(&doh_note),
        );
        let lv = prefs::unblock_level();
        let set_unblock = |l: i64| {
            move || {
                prefs::set_unblock_level(l);
                cbtv::net::settings_changed();
            }
        };
        self.option(
            "Vượt chặn nhà mạng",
            vec![choice("Tắt", lv == 0, set_unblock(0)), choice("Mức 1", lv == 1, set_unblock(1)), choice("Mức 2", lv == 2, set_unblock(2))],
            Some(
                "Trang không vào được / quá thời gian chờ (nhà mạng chặn theo tên trang, vd phimmois.fun): bật Mức 1 — app chia nhỏ gói \
                 bắt tay HTTPS (cả yêu cầu HTTP thường). Ở Mức 1 mà trang vẫn không trả lời thì app tự thử Mức 2 cho riêng trang đó và nhớ \
                 7 ngày. Trang / ảnh bìa bị chặn bằng DNS (trả 0.0.0.0, 127.0.0.1 hay địa chỉ trang cảnh báo) thì app tự hỏi lại địa chỉ \
                 qua DNS mã hoá. Không cần VPN.",
            ),
        );
        b.add(&ui::row(&[ui::pill(&crate::hooks::proxy_label(), || app().push(crate::hooks::proxy_page())).upcast()]));
        let au = prefs::auto_update();
        self.option(
            "Tự kiểm tra bản mới",
            vec![choice("Bật", au, || prefs::set_auto_update(true)), choice("Tắt", !au, || prefs::set_auto_update(false))],
            None,
        );
        b.add(&ui::row(&[
            ui::pill("Kiểm tra cập nhật", || ui::local(crate::startup::check_update(true))).upcast(),
            ui::pill("Xoá lịch sử xem", || {
                ui::local(async {
                    if !app().confirm("Xoá cả lịch sử xem?", "Xoá danh sách Xem tiếp và chỗ đang xem của mọi tập.", "Xoá hết", "Huỷ").await {
                        return;
                    }
                    prefs::clear_history();
                    crate::startup::sync_soon(0);
                    app().toast("Đã xoá lịch sử xem.");
                });
            })
            .upcast(),
            ui::pill("Xoá dữ liệu duyệt web", || {
                ui::local(async {
                    if !app().confirm("Xoá dữ liệu duyệt web?", "Xoá cookie, bộ nhớ đệm của các trang phim và ảnh bìa đã lưu.", "Xoá", "Huỷ").await {
                        return;
                    }
                    crate::player::clear_browsing_data();
                    app().toast("Đã xoá dữ liệu duyệt web.");
                });
            })
            .upcast(),
        ]));

        let h = ui::header("Phím tắt");
        h.set_margin_top(px(26.0));
        b.add(&h);
        b.add(&ui::dim(
            "Mũi tên: di chuyển  •  Enter: chọn  •  Esc / Backspace / Back của remote: quay lại  •  F11: toàn màn hình  •  Ctrl+F: tìm kiếm\n\
             Khi xem phim: Space / Enter / K: phát / dừng  •  ◀ ▶ (J / L): tua  •  ▲: thời gian  •  ▼ / M: menu (server, tập, tốc độ…)  •  \
             N / P (PageDown / PageUp): tập sau / trước  •  0–9: nhảy tới 0–90%  •  F: toàn màn hình  •  Esc: thoát phim\n\
             Chuột: rê để hiện thanh điều khiển, bấm vào phim để phát / dừng, bấm đúp để toàn màn hình.",
        ));
        let h = ui::header("Giới thiệu");
        h.set_margin_top(px(26.0));
        b.add(&h);
        b.add(&ui::dim(&format!("Côn Bằng TV cho Linux — bản {}", cbtv::update_check::current_version())));
        b.add(&ui::row(&[ui::pill(&format!("Cài lên tivi / mời bạn bè — mã Downloader {}", cbtv::invite::CODE), || {
            app().push(super::invite::new())
        })
        .upcast()]));
        b.add(&ui::row(&[
            ui::pill("Liên hệ: t.me/tivikit", || ui::open_browser("https://t.me/tivikit")).upcast(),
            ui::pill("Miễn trừ trách nhiệm & quyền riêng tư", || {
                ui::local(async {
                    crate::startup::show_disclaimer(false).await;
                });
            })
            .upcast(),
            ui::pill("Trang phát hành", || ui::open_browser(cbtv::update_check::RELEASES_PAGE)).upcast(),
        ]));
        let d = ui::dim(&format!("Dữ liệu của app: {}", prefs::data_dir().display()));
        d.set_margin_top(px(6.0));
        b.add(&d);
    }
}

impl Page for SettingsPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
    }

    fn returned(self: Rc<Self>) {
        self.build();
    }
}
