# Nandemo-FBX

MMD（PMX / VMD）や VRM / VRMA を FBX に変換するツールです。Rust で書かれていて、外部ソフト（Blender や FBX SDK）なしで動きます。

> 開発初期の段階です。今のところ PMX の静的メッシュ（形状・法線・UV・マテリアルの色）だけを FBX に書き出せます。

## 使い方

```sh
cargo run --release -- model.pmx                  # model.fbx を書き出す
cargo run --release -- model.pmx -o out/model.fbx
cargo run --release -- model.pmx -m dance.vmd     # モーションは読み込みのみ（書き出しは未対応）
```

## 対応状況

| 形式 | 読み込み | FBX への書き出し |
|---|---|---|
| PMX | 頂点・面・テクスチャ・マテリアル・ボーン・モーフ | メッシュ・マテリアルの色 |
| VMD | ボーン・モーフのキーフレーム | 未対応 |
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

- [ ] ボーン（LimbNode）・スキン・バインドポーズの書き出し
- [ ] モーフ（BlendShape）の書き出し
- [ ] テクスチャの書き出し
- [ ] アニメーションの書き出し（VMD のベジェ補間、IK・付与親の焼き込み）
- [ ] VRM / VRMA の読み込み
- [ ] ボーン名の対応表とリターゲット
- [ ] 大きな配列の zlib 圧縮

## 注意

MMD モデルや VRM の多くには利用規約があり、改変や形式の変換を禁止しているものもあります。変換する前に、各モデルの規約を確認してください。

## ライセンス

[MIT](LICENSE)
