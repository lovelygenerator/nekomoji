use super::*;

fn text(a: Action) -> String {
    match a {
        Action::Print(s) => s,
        other => panic!("{other:?}"),
    }
}

#[test]
fn 文を入力すると全体を表示する() {
    let mut r = Repl::default();
    let out = text(r.handle("あいう"));
    assert!(out.contains(" 1 | あいう"), "{out}");
}

#[test]
fn 幅を変えると組み直す() {
    let mut r = Repl::default();
    r.handle("あいうえお");
    let out = text(r.handle(":width 2"));
    assert!(out.contains(" 1 | あい\n 2 | うえ\n 3 | お"), "{out}");
}

#[test]
fn writing_で縦書きと横書きを切り替える() {
    let mut r = Repl::default();
    r.handle(":width 2");
    r.handle("あいうえ");
    let out = text(r.handle(":writing tate"));
    assert!(out.contains("縦書き"), "{out}");
    assert!(out.contains("  2  1\n"), "{out}");
    assert!(out.contains(" う あ\n え い\n"), "{out}");
    let out = text(r.handle(":writing yoko"));
    assert!(out.contains(" 1 | あい"), "{out}");
    assert!(text(r.handle(":writing naname")).contains("yoko か tate"));
}

#[test]
fn state_で取り出した状態に_restore_で戻せる() {
    let mut r = Repl::default();
    r.handle(":speed 10");
    r.handle("あいうえお[p]かき");
    r.handle(":step 0.25");
    let st = text(r.handle(":state"));
    assert_eq!(st, "p0 t0.25");
    r.handle(":next");
    r.handle(":next");
    let out = text(r.handle(&format!(":restore {st}")));
    assert!(out.contains("ページ 1/2  t=0.25s  3/5 文字"), "{out}");
    assert!(text(r.handle(":restore p9 t0")).contains("戻せません"));
    assert!(text(r.handle(":restore x")).contains("読めません"));
}

#[test]
fn step_で文字が増え出来事が出る() {
    let mut r = Repl::default();
    r.handle(":speed 10");
    r.handle("あい[voice v1]う[p]え");
    let out = text(r.handle(":step 0.15"));
    assert!(out.contains(" 1 | あい\n"), "{out}");
    assert!(!out.contains("♪"), "{out}");
    let out = text(r.handle(":step 0.1"));
    assert!(out.contains("♪ voice v1"), "{out}");
    let out = text(r.handle(":step 1"));
    assert!(out.contains("入力待ち"), "{out}");
    let out = text(r.handle(":next"));
    assert!(out.contains("ページ 2/2"), "{out}");
}

#[test]
fn 変数を設定して展開する() {
    let mut r = Repl::default();
    r.handle("[var name]だにゃ");
    let out = text(r.handle(":set name ミケ"));
    assert!(out.contains("ミケだにゃ"), "{out}");
}

#[test]
fn 解析エラーは位置を示す() {
    let mut r = Repl::default();
    let out = text(r.handle("あ[x]"));
    assert!(out.contains("エラー 1:2: 知らないタグ [x]"), "{out}");
}

#[test]
fn quit_と_play() {
    let mut r = Repl::default();
    assert_eq!(r.handle(":quit"), Action::Quit);
    assert!(matches!(r.handle(":play"), Action::Print(_)));
    r.handle("あ");
    assert_eq!(r.handle(":play"), Action::Play);
}

#[test]
fn 猫モードで押して落とすのとカーソルを確かめられる() {
    let mut r = Repl::default();
    r.handle(":punch 0");
    r.handle(":burst 0");
    r.handle(":knock 1");
    let out = text(r.handle("あいう[p]え"));
    assert!(out.contains("猫 seed 0 落とす 1 爆散 0 パンチ 0"), "{out}");
    assert!(out.contains("「う」を押して落とす"), "{out}");
    // 動きなしでは猫は何もしない
    let out = text(r.handle(":motion off"));
    assert!(
        out.contains("動き: なし") && out.contains("何もしない"),
        "{out}"
    );
    assert!(!out.contains("押して落とす"), "{out}");
    assert!(text(r.handle(":motion fast")).contains("full・reduced・off"));
    let out = text(r.handle(":motion full"));
    assert!(out.contains("「う」を押して落とす"), "{out}");
    let out = text(r.handle(":step 0.1"));
    assert!(!out.contains("🐈") && !out.contains("🐾"), "{out}");
    r.handle(":next");
    let out = text(r.handle(":step 1"));
    assert!(out.contains(" 1 | あい") && !out.contains("う"), "{out}");
    assert!(out.contains("🐾"), "{out}");
    let out = text(r.handle(":neko off"));
    assert!(!out.contains("猫"), "{out}");
}

#[test]
fn バックログは読んだページまでを出す() {
    let mut r = Repl::default();
    r.handle("[ruby ねこ]猫[/ruby]だ[p]にゃ");
    let out = text(r.handle(":backlog"));
    assert_eq!(out, "[ページ 1]\n｜猫《ねこ》だ");
    r.handle(":next");
    r.handle(":next");
    let out = text(r.handle(":backlog"));
    assert!(out.ends_with("[ページ 2]\nにゃ"), "{out}");
}

#[test]
fn 読み上げ用のテキストはルビを読みにする() {
    let mut r = Repl::default();
    r.handle("[ruby ねこ]猫[/ruby][icon paw]だ");
    assert_eq!(text(r.handle(":speech")), "[ページ 1]\nねこだ");
}

#[test]
fn l_ではクリック待ちになり_next_で同じページの続きを出す() {
    let mut r = Repl::default();
    r.handle("あい[l]うえ");
    let out = text(r.handle(":step 1"));
    assert!(
        out.contains(" 1 | あい") && out.contains("クリック待ち"),
        "{out}"
    );
    let out = text(r.handle(":next"));
    assert!(
        out.contains(" 1 | あいう") && out.contains("ページ 1/1"),
        "{out}"
    );
    let out = text(r.handle(":next"));
    assert!(
        out.contains(" 1 | あいうえ") && out.contains("終了"),
        "{out}"
    );
}
