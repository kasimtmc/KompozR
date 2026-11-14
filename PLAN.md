# KompozR planı

KompozR, Kompoz'un (C++20) Rust sürümü: aynı zihinsel model (oluşturulabilir işlevler, durum, akıllı yeniden
oluşturma, değiştiriciler), aynı adlar, Compose'unkiler gibi görünen ve davranan Material 3 bileşenleri; Linux,
Windows, macOS, Android ve web tek kod tabanından. Hedef, C++ sürümünün bugün yaptığı her şeyi Rust'ta aynı
davranışla yapmak; C++'ta bilinen eksikler (aşağıda "C++'tan devralınmayacak borçlar") baştan doğru kurulur.

## 1. Kararlar (2026-11-14)

| Konu | Karar |
|---|---|
| Yazım | İki yazım. Temel: Rust ve `#[kompozable]` yordamsal makrosu (Compose derleyici eklentisinin işini yapar). Üstünde: Kotlin/Compose sözdizimi, `kompoz! { … }` makrosu içinde (C++'taki `kompozc`'nin karşılığı) |
| Platform | Hepsi bizim: Wayland, X11, Win32, AppKit, Android, web katmanları ve GPU arka uçları. Pencere soyutlaması ya da hazır çizim kütüphanesi (winit, wgpu, SDL) yok |
| GPU | Vulkan, Metal, WebGPU (`yapılacaklar` notuna göre OpenGL ES / WebGL 2 yok) |
| Metin | Saf Rust: `skrifa`/`swash` (yazı tipi okuma, glif çizimi), `harfrust` (şekillendirme), `fontique` (sistem yazı tipleri, yedek yazı tipleri) |
| Lisans | Apache 2.0 (C++ sürümüyle aynı) |
| Dil sürümü | Rust 2024 baskısı, kararlı derleyici; en düşük sürüm ilk yayında sabitlenir |
| Rust yazımında çağrılar | A biçimi (Aşama 0 prototipinden): `#[kompozable]` gövdesinde Kotlin'deki gibi adlı bağımsız değişkenler (`ad = değer`), varsayılanlar ve sondaki kapanış; büyük harfli çağrılar her zaman, küçük harfli işlev ve yöntem çağrıları adlı bir değişken olduğunda kurucuya çevrilir |
| Rust yazımında adlar | İşlevler Compose'daki gibi (`Column`, `Text`), parametre ve yöntemler Rust'taki gibi (`vertical_arrangement`, `fill_max_width()`) |
| Gönderme | Her aşama bitince `main`'e gönderilir, sürekli tümleştirme yeşil olmalı |
| Kapanışların yakaladığı (Aşama 3) | `#[kompozable]` gövdesinde oluşturulabilir ve adlı çağrılara verilen kapanışlar ve `async` blokları, kullandıkları yerel değişkenlerin kopyasını alır (Kotlin'deki paylaşımın karşılığı; durumlar ve `Rc`'ler paylaşır). Başka bir kapanışın içindeki `move` kapanışları da kopyalar. Gövdenin en üstünde açıkça `move` yazılan kapanış, düz Rust'taki gibi taşır |
| Birimler (Aşama 4) | Compose'unki: yerleşim ve çizim piksel (kısıtlar ve ölçüler tam sayı piksel), `Dp`, `Sp`, `DpOffset`, `DpSize` ayrı türler, `Rect` sol/üst/sağ/alt. (C++'ta her şey dp idi.) |
| Arayüz çekirdeğinin yapısı (Aşama 5) | Compose'unki: düğüm temelli değiştiriciler (`Modifier.Node`, zincir karşılaştırılarak düğümler yeniden kullanılır), ölçme ve yerleştirme ayrı geçişler, gerçek iç boyutlar, sağdan sola `place_relative` ile, tıklama/kaydırma/odak değiştirici düğümleri, testler anlam ağacını sorgular. Davranış ve testler C++'tan. (C++'ta değiştiriciler düz bir öğe listesiydi, ölçme ile yerleştirme tek geçişti, tıklama ve kaydırmayı ana makine yürütüyordu.) |
| Foundation'ın bir kısmı (Aşama 5) | `kompoz-foundation` Aşama 5'te açılır: Box, Row, Column, Spacer, dizilim, dolgu ve boyut değiştiricileri, clickable, focusable, selectable, toggleable (Compose'daki gibi eksiksiz). Kaydırma, tembel listeler ve geri kalanı Aşama 10'da |
| Metinden önceki testler (Aşama 5) | `kompoz-test`'te C++'taki `FakeTextMeasurer` formülüyle ölçülen tek satırlık bir sınama `Text`'i; satır kırma isteyen testler Aşama 6'ya kalır, Aşama 6'da gerçek `Text`'e geçilir |
| Oluşturulabilir türleri (Aşama 3) | Hiçbir şey döndürmeyen `#[kompozable]` yeniden başlatılabilir; bütün parametreleri `PartialEq` ise atlanabilir. `#[kompozable(inline)]` ve değer döndüren oluşturulabilir, çağıranın kapsamında çalışır (Compose'un satır içi oluşturulabilirleri gibi). Oluşturulabilirler yalnızca oluşturulabilir gövdelerinden (`#[kompozable]`, `kompozable_lambda!`, `kompozable_block!`) çağrılır; kimlik çağrı yerinden gelir |

"Hepsi bizim" çizgisi: işletim sisteminin kendi arayüzüne erişmek için yalnızca ince bağlama paketleri kullanılır
(`wayland-client` + `wayland-protocols`, `x11rb`, `windows`, `objc2-*`, `jni`/`ndk-sys`, `web-sys`, `ash`). Bunlar
C++'taki sistem başlık dosyalarının karşılığıdır; pencere, olay döngüsü, giriş yöntemi, çizim, erişilebilirlik
köprüleri kendi kodumuzdur.

## 2. Paket yapısı (Cargo çalışma alanı)

Compose'un modül ayrımı izlenir; Compose bilen biri neyin nerede olduğunu tahmin edebilsin.

```
kompoz                  şemsiye paket: her şeyi dışa verir, `use kompoz::prelude::*`
kompoz-runtime          anlık görüntüler (MVCC), yuva tablosu, oluşturucu, yeniden başlatma kapsamları, remember,
                        etkiler, KompozitionLocal, rememberSaveable, derivedStateOf, durum listeleri/haritaları
kompoz-coroutines       kendi yürütücümüz (kare saatine bağlı), Job ve yapılandırılmış eşzamanlılık, delay,
                        withTimeout, async/await, Flow/StateFlow/SharedFlow, Channel
kompoz-macros           #[kompozable], #[derive(Route)], #[derive(Saveable)] …
kompoz-kotlin           Kotlin yazımının çevirisi (sözcük çözümleyici, ayrıştırıcı, Rust üretici) — kitaplık
kompoz-kotlin-macro     kompoz! { } makrosu (kompoz-kotlin'in ince sarmalayıcısı)
kompoz-graphics         Color ve renk uzayları, HCT ve tonal paletler, dinamik renk, geometri, Path (+ işlemler,
                        efektler), Brush, Shape, ImageVector, ImageBitmap, görüntü çözme
kompoz-text             AnnotatedString, yazı tipleri, şekillendirme, çift yönlü metin, satır kırma, TextMeasurer
kompoz-ui               LayoutNode ağacı, Constraints, Modifier düğümleri, ölçme → yerleştirme → çizme (artımlı),
                        DrawScope, graphicsLayer, görüntü listesi, işaretçi girişi, odak, tuşlar, anlam ağacı,
                        KompozHost, açılır katman, iç içe kaydırma, pencere kenar boşlukları, sağdan sola
kompoz-foundation       Box/Row/Column/FlowRow/FlexBox/Grid, Layout/SubkompozLayout, kaydırma fiziği, tembel
                        listeler ve ızgaralar, sayfalayıcılar, metin alanı, tıklanabilirlik, sürükle-bırak
kompoz-animation        özellikler, Animatable, animate*AsState, geçişler, AnimatedVisibility/Content,
                        Crossfade, animateContentSize, SharedTransitionLayout
kompoz-material3        tema, tipografi, şekiller, Material 3 1.4 + Expressive bileşenlerinin hepsi, simgeler
kompoz-navigation       NavHost, türlü rotalar, derin bağlantılar, iç içe grafikler
kompoz-lifecycle        Lifecycle, ViewModel, SavedStateHandle
kompoz-gpu              çizici çekirdeği + aygıt katmanı: Vulkan (ash), Metal (objc2-metal), WebGPU (web-sys);
                        Slang gölgelendiricilerinden üretilmiş SPIR-V / MSL / WGSL
kompoz-platform         uygulama çekirdeği (kare döngüsü, olaylar, kendi başlık çubuğumuz) + platformlar:
                        wayland, x11, win32, appkit, android (Java tarafıyla), web, headless
kompoz-test             KompozTestRule (başsız arayüz testleri), altın görüntü karşılaştırması
xtask                   geliştirme komutları: paketleme, gölgelendirici üretimi, belgeler, ekran görüntüleri
examples/showcase       C++ vitrininin tamamı
editors/                dil sunucusu ve düzenleyici eklentileri (Kotlin yazımı için)
docs/guide/{en,tr}      rehber, iki dilde
```

## 3. Rust'a özgü tasarım

- **Kimlik.** Her oluşturulabilir işlev `#[track_caller]` olur; çağrı yeri (`Location::caller()`) ve üst
  öğedeki sıra kimliktir. C++'taki `CallSite::current()` (`std::source_location`) ile aynı fikir, ama elle
  parametre eklemek yok: makro ekler.
- **Yeniden başlatma ve atlama.** `#[kompozable]` gövdeyi bir kapsama sarar; kapsam okuduğu durum değişince tek
  başına yeniden çalışır. Bağımsız değişkenleri `PartialEq` olan işlev, değişkenleri öncekine eşitse atlanır;
  karşılaştırılamayan bir değişken (kapanış vb.) işlevi yeniden başlatılabilir ama atlanamaz yapar (makro bunu
  derleme zamanında seçer).
- **Sahiplik.** `MutableState<T>` kopyalanabilir küçük bir tanıtıcıdır (anlık görüntü deposundaki kayda bir
  kimlik). Kapanışlar `move` ile yakalar; C++'ta `[&]` ile yakalamanın yol açtığı sarkan başvuru hataları
  (vitrinin Android'de açılışta çökmesi gibi) Rust'ta derlenmez bile.
- **İş parçacıkları.** Oluşturma arayüz iş parçacığında; anlık görüntüler sayesinde arka plandan durum yazmak
  güvenli (C++'ta 6. adımda kurulan MVCC'nin karşılığı, `Send`/`Sync` sınırlarıyla derleyicinin denetiminde).
- **Eşyordamlar.** Rust'ın `async`/`await`'i; kendi tek iş parçacıklı yürütücümüz kare saatine bağlı
  (`delay`, `withFrameNanos`). İptal, geleceği (future) bırakarak ve yapılandırılmış kapsamlarla.
- **Birimler.** Rust yazımında `16.dp()`, Kotlin yazımında `16.dp`.
- **Modifier.** Compose'daki gibi düğüm temelli (`Modifier.Node`); zincirde değişmeyen düğümler yeniden
  kullanılır.

### Aşama 0'da senin seçeceğin iki şey

1. **Rust yazımında adlandırma.** Önerim: işlev adları Compose'daki gibi (`Column`, `Text`, `MaterialTheme`),
   parametre adları Rust'taki gibi (`vertical_arrangement`). Kotlin yazımı zaten Compose'un adlarını birebir verir.
2. **Rust yazımında adlı ve isteğe bağlı parametreler.** Rust'ta adlı parametre yok. `#[kompozable]` gövdesinde
   `Column(modifier = m, vertical_arrangement = a, || { … })` geçerli Rust sözdizimi olduğundan makro bunu
   varsayılanlarla dolu bir çağrıya çevirebilir; makro dışındaki düz Rust kodu için ayrıca bir yol gerekir
   (seçenek yapısı ya da kurucu zinciri). İki üç biçimi gerçek kodla (Counter + senin Scaffold örneğin) yazıp
   önüne koyacağım; sen seçeceksin.

### Kotlin yazımı nasıl çalışacak

`kompoz! { @Kompozable fun Counter() { var count by remember { mutableStateOf(0) } … } }` bir `.rs` dosyasının
içinde durur. Makro, içeriğin özgün metnini (`Span::source_text`) kendi Kotlin sözcük çözümleyicisiyle yeniden
okur; böylece `"${if (a) "x" else "y"}"` gibi iç içe dizgi şablonları da doğru çözülür, hatalar Kotlin satırlarını
gösterir. Bilinen tek sınır: Rust'ın sözcük çözümleyicisinin hiç kabul etmediği karakterler (Kotlin'in ters tırnaklı
adları, `` `fun name` ``) makronun içine yazılamaz. Düzenleyici desteği için C++'taki `kompozc --lsp`'nin karşılığı
bir dil sunucusu, `rust-analyzer`'ın önünde durur.

## 4. Aşamalar

Her aşama: C++ sürümündeki karşılık gelen testler Rust'a taşınır ve geçer, `cargo clippy` ve `cargo fmt` temiz,
güvensiz (`unsafe`) kod Miri'den geçer, iş parça parça ve her parça derlenip test edilerek işlenir. Bir aşama
bitmeden sonrakine geçilmez.

**Aşama 0 — Temel ve yazım prototipleri.** Çalışma alanı, lisans, `.gitignore`, sürekli tümleştirme (GitHub
Actions: Linux'ta derleme, test, clippy, fmt). Yukarıdaki iki seçim için prototipler. *Doğrulama:* prototipler
derlenir; sen biçimi seçersin.

**Aşama 1 — Çalışma zamanı.** Anlık görüntüler, yuva tablosu, oluşturucu, kapsamlar ve okuma izleme, `remember`
(çok anahtarlı), `key`, etkiler Compose'un sırasında (`DisposableEffect`, `SideEffect`, `RememberObserver`),
`KompozitionLocal`, `derivedStateOf`, `mutableStateListOf`/`mutableStateMapOf`, `rememberUpdatedState`, değişim
politikaları, `rememberSaveable` ve `Saver`. *Doğrulama:* `kompoz_tests.cpp`'nin çalışma zamanı testleri.

**Aşama 2 — Eşyordamlar ve akışlar.** Yürütücü, kare saati, `delay`, `awaitFrame`, `Job`, `async`/`Deferred`,
`coroutineScope`/`supervisorScope`, `withTimeout(OrNull)`, `LaunchedEffect`, `rememberCoroutineScope`,
`produceState`, `snapshotFlow`, `Flow`/`StateFlow`/`SharedFlow`/`Channel`, `collectAsState`.
*Doğrulama:* C++'taki eşyordam ve akış testleri.

**Aşama 3 — `#[kompozable]`.** Kimlik, yeniden başlatma, atlama, Aşama 0'da seçilen parametre biçimi.
*Doğrulama:* `kompozable_tests.cpp`'nin karşılıkları + derleme hatası testleri (`trybuild`).

**Aşama 4 — Grafik temelleri.** Renk uzayları, HCT, tonal paletler, dinamik renk şemaları, geometri, `Path` ve
yol işlemleri, yol efektleri, `Brush`, `Shape` (Material şekilleri dahil), `ImageVector` (vektör çizim XML'i),
`ImageBitmap`, PNG/JPEG/WebP çözme (`png`, `zune-jpeg`, `image-webp`). *Doğrulama:* C++ testlerinin karşılıkları;
HCT ve renk sonuçları C++ sürümüyle birebir.

**Aşama 5 — Arayüz çekirdeği.** Yerleşim düğümleri, kısıtlar, artımlı ölçme ve yerleştirme, iç boyutlar,
`DrawScope`, `graphicsLayer`, görüntü listesi (yalnızca değişeni yeniden çizme), işaretçi girişi (düğüm başına
eşyordam), hareketler, odak ve tuşlar, anlam ağacı, `KompozHost`, açılır katman, iç içe kaydırma, kenar
boşlukları, sağdan sola. Başsız `KompozTestRule`. *Doğrulama:* C++'taki yerleşim, hareket, odak, anlam testleri.

**Aşama 6 — Metin.** Sistem yazı tipleri ve yedek yazı tipleri, emoji, `harfrust` ile şekillendirme, tam Unicode
çift yön algoritması (C++'taki basitleştirilmiş sürüm yerine), satır kırma ve sözcük sınırları, `AnnotatedString`,
`TextMeasurer`, seçim, otomatik boyut, taşma. *Doğrulama:* C++ metin testleri + Arapça, Hintçe, emoji dizileri.

**Aşama 7 — GPU çizici.** Aygıt katmanı (C++'taki `device.hpp`'nin karşılığı) ve çizici çekirdeği; önce Vulkan.
Slang gölgelendiricileri olduğu gibi taşınır, `xtask shaders` SPIR-V / MSL / WGSL üretir (C++'taki `shaders.py`
gibi, üretilenler depoya işlenir). Glif atlası, yol geometrisi, bulanık gölgeler, katmanlar.
*Doğrulama:* `gpu_tests.cpp`'nin karşılıkları; çıktılar C++'ın `tests/golden` görüntüleriyle piksel piksel
karşılaştırılır (lavapipe ve gerçek GPU).

**Aşama 8 — İlk platform: Linux.** Uygulama çekirdeği, Wayland (xdg-shell, metin girişi v3, işaretçi hareketleri,
tablet, kendi başlık çubuğumuz), X11, başsız platform; koyu tema ve vurgu rengi XDG portalından, AT-SPI
erişilebilirlik köprüsü, pano, sistem yazı tipi ölçeği, dil. *Doğrulama:* ekranda çalışan Counter; platform
testleri.

**Aşama 9 — Kotlin yazımının çekirdeği.** `kompoz! { }`: sözcük çözümleyici, ayrıştırıcı, Rust üretici; Counter ve
ilk bileşenler Kotlin yazımıyla. Bundan sonra her yeni bileşen iki yazımla da test edilir.
*Doğrulama:* C++'taki `kompozc` testlerinin karşılıkları.

**Aşama 10 — Foundation.** Bütün yerleşimler, `SubkompozLayout` (gerçek alt oluşturma, baştan), tembel listeler,
ızgaralar, kademeli ızgaralar, sayfalayıcılar, döngüler, Android'in savurma eğrisi ve esneyen aşırı kaydırma,
yapışma, metin alanı ve yazılım klavyesi, tıklanabilirlik ve dalga efekti, etkileşim kaynakları, çapalı
sürükleme, sürükle-bırak. *Doğrulama:* C++ testleri.

**Aşama 11 — Animasyon.** Hepsi (bkz. paket yapısı). *Doğrulama:* C++ testleri.

**Aşama 12 — Material 3.** `docs/components.md`'deki her bileşen, Compose Material 3 1.4 ve Expressive
bileşenleriyle aynı ölçüler ve davranış. *Doğrulama:* C++ testleri + vitrin ekran görüntüleri C++ sürümünün
ekran görüntüleriyle karşılaştırılır.

**Aşama 13 — Navigasyon, yaşam döngüsü, ViewModel.** Türlü rotalar (`#[derive(Route)]`), derin bağlantılar, iç
içe grafikler, `SavedStateHandle`, süreç bitip geri gelince durumun korunması.

**Aşama 14 — Kotlin yazımının tamamı.** C++'taki `kompozc`'nin kapsadığı her şey: veri, enum, mühürlü ve iç
sınıflar, arayüzler, nesneler, eşlik eden nesneler, soysal türler, `suspend fun`, `try`/`throw`, işleçler, uzantı
işlevleri ve özellikleri, devredilen özellikler, aşırı yüklemeler. Dil sunucusu ve düzenleyici eklentileri
(CLion, VS Code, Kate).

**Aşama 15 — Diğer platformlar.** Android (C++'taki Java tarafı taşınır: giriş bağlantısı, erişilebilirlik,
kenar boşlukları, dinamik renk, geri hareketi; `cargo-ndk`; telefonda deneme), Windows (Win32, UI Automation,
Vulkan), web (wasm32, WebGPU), macOS (AppKit, Metal; sürekli tümleştirmede).
*Doğrulama:* her platformda vitrin; sürekli tümleştirme matrisi.

**Aşama 16 — Vitrin, araçlar, belgeler.** Vitrinin tamamı (C++'taki bütün ekranlar), kaydedince yeniden derleyip
durumu koruyarak yeniden başlatma, önizlemeler, betikli ekran görüntüleri, kare profilleme (`KOMPOZ_PROFILE`,
`KOMPOZ_TRACE`), her platform için paketleme, README ve rehber iki dilde, değişiklik günlüğü.

## 5. C++'tan devralınmayacak borçlar

C++ sürümünün "bilinen sınırlamaları"ndan Rust'ta baştan doğru kurulacak olanlar:

- Gerçek alt oluşturma (C++'ta tembel listeler ve `BoxWithConstraints` kare başına sınırlı geçişle çalışıyordu).
- Tembel listelerde `contentType`'a göre oluşturma havuzu.
- Tam Unicode çift yön algoritması (gömme ve geçersiz kılmalar dahil).
- İç içe kaydırmada `onPreFling`/`onPostFling`'in Compose'daki gibi askıya alınması.
- Yeniden oluşturma vurgulayıcısının sayaçlarının 3 saniyede sıfırlanması.

## 6. Senden onay beklediğim noktalar

1. Bu aşama sırası (çekirdekten yukarı; ekranda ilk görüntü Aşama 8'de, Kotlin yazımı Aşama 9'da başlıyor).
2. "Hepsi bizim" çizgisi: sistem arayüzleri için ince bağlama paketleri kullanmak (2. bölümdeki liste).
3. OpenGL ES olmadığı için: Vulkan'sız eski Android cihazlar (kabaca Android 7 öncesi ve bazı ucuz cihazlar) ve
   WebGPU'su olmayan tarayıcılar desteklenmeyecek.
4. Kotlin yazımının `.rs` dosyalarında `kompoz! { }` içinde durması (C++'taki gibi kaynak dosyanın içinde).
