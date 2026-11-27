# Nandemo-FBX

MMD（PMX / VMD）や VRM / VRMA を FBX に変換するツールです。Rust で書かれていて、外部ソフト（Blender や FBX SDK）なしで動きます。

> 開発初期の段階です。PMX のメッシュ・ボーン・スキンと、VMD のボーンアニメーションを FBX に書き出せます。
> IK と付与親はまだ焼き込まないので、足 IK を使うモーションでは足が動きません。

## 使い方

```sh
cargo run --release -- model.pmx                         # model.fbx を書き出す
cargo run --release -- model.pmx -o out/model.fbx
cargo run --release -- model.pmx -m dance.vmd            # モデル＋アニメーション
cargo run --release -- model.pmx -m dance.vmd -e anim    # ボーンとアニメーションだけ
cargo run --release -- model.pmx -m dance.vmd -e model   # モーションを無視してモデルだけ
```

`-e` / `--export` で書き出す内容を選べます。

| 値 | 書き出す内容 |
|---|---|
| `all`（既定） | メッシュ・マテリアル・ボーン・スキン・アニメーション（`-m` 指定時） |
| `model` | メッシュ・マテリアル・ボーン・スキン |
| `anim` | ボーンとアニメーションだけ（`-m` が必要）。モデルはボーン構造を得るために使います |

アニメーションは毎フレームの値を焼き込みます（30 fps）。モデルにないボーンのモーションは無視し、警告を出します。

## 対応状況

| 形式 | 読み込み | FBX への書き出し |
|---|---|---|
| PMX | 頂点・面・テクスチャ・マテリアル・ボーン・モーフ | メッシュ・マテリアルの色・ボーン・スキン・バインドポーズ |
| VMD | ボーン・モーフのキーフレーム（ベジェ補間付き） | ボーンアニメーション |
| VRM | 未対応 | 未対応 |
| VRMA | 未対応 | 未対応 |

## 構成

```
crates/
  nandemo-core  中間表現（IR）とバイナリリーダー
  nandemo-pmx   PMX リーダー（PMX → IR）
  nandemo-vmd   VMD リーダー（VMD → IR）
  nandemo-fbx   FBX ライター（IR → FBX バイナリ 7.4）
  nandemo-cli   コマンドラインツール（実行ファイル名は nandemo-fbx）
```

各リーダーは入力を IR に変換し、FBX ライターは IR だけを見て書き出します。座標系と単位の変換はリーダー側で一度だけ行います。

IR の規約（glTF / VRM 1.0 と同じ）:

- 右手系・Y-up・モデルの正面は +Z
- 単位はメートル（MMD の 1 単位 = 0.08 m として換算）
- UV は左上原点、三角形は反時計回りが表面

FBX の出力は Y-up・正面 +Z・センチメートル単位です。

## テスト

```sh
cargo test
```

FBX ライターのテストでは、書き出したファイルを [ufbx](https://github.com/ufbx/ufbx)（厳密モード）で読み戻して検証しています。

## 今後の予定

- [x] ボーン（LimbNode）・スキン・バインドポーズの書き出し
- [x] ボーンアニメーションの書き出し（VMD のベジェ補間を焼き込み）
- [x] 大きな配列の zlib 圧縮
- [ ] IK・付与親の焼き込み
- [ ] モーフ（BlendShape）とモーフアニメーションの書き出し
- [ ] テクスチャの書き出し
- [ ] 焼き込んだキーの間引き
- [ ] VRM / VRMA の読み込み
- [ ] ボーン名の対応表とリターゲット

## 注意

MMD モデルや VRM の多くには利用規約があり、改変や形式の変換を禁止しているものもあります。変換する前に、各モデルの規約を確認してください。

## ライセンス

[MIT](LICENSE)
