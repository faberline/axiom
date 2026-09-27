# Layer SFT 研究掃描（2026-09-23）

本文整理 `layer-sft-003` 之後做的文獻掃描。每篇 paper 只記一件事：它量到的結論，以及這個結論對 SOM 下一步有什麼影響。文末是依這些結論排出的實驗順序。record 格式見 [layer-records.md](layer-records.md)，四層架構見 [architecture.md](architecture.md)。

## 出發點：003 的量測

設定：Qwen2.5-Coder-1.5B-Instruct-4bit，LoRA 掛在 top 16 層（rank 16）。訓練集 81 個 family，held-out 20 個。解碼用 greedy，加 repetition penalty 1.1、context 20。

| 量測 | 002（每層只看上一層） | 003（累積式 conditioning） |
|---|---|---|
| val loss l1 / l2 / l3 | 1.79 / 0.47 / 0.69 | 1.74 / 0.26 / 0.52 |
| chain 到得了 fixture | 0/20（全卡在 l3） | 7/20 |
| L2｜gold L1 合法 | 17/20 | 20/20 |
| L3｜gold L1、L2 到得了 fixture | 0/20 | 8/20 |
| fixture 通過 | 0/18 | 0/18 |

在 gold 上游下，L2 已經 20/20 合法，瓶頸只剩 L3。L3 的失敗有四類：

1. **格式錯誤**：JSON 字串沒收尾、括號不配對。ops 裡 67% 的字元是 `INSERT_BLOCK` 的字面原始碼，而這些程式碼要跳脫之後塞進 JSON 字串。
2. **長距離重複**：同一段程式或同一條 op 反覆出現。penalty 只看最近 20 個 token，擋不住。
3. **引用錯誤**：ops 用到 L2 沒宣告的 block 名稱，或宣告了卻沒插入。
4. **程式本身錯**：漏 import、呼叫不存在的名稱。

## 文獻與對 SOM 的意義

### 1. 程式碼包在 JSON 裡，品質會下降（對應失敗類別 1）

- **Aider, "LLMs are bad at returning code in JSON"（2024-08）**：用 133 題 Exercism Python 測，程式改放 JSON 後分數普遍下降，例如 Claude 3.5 Sonnet 從約 86% 降到約 81%，DeepSeek-Coder V2 從約 85% 降到約 76%。OpenAI 的 strict JSON 模式也沒有改善。原因有兩個：跳脫本身就會出錯（例如 unterminated string），模型還得分神維持 JSON 結構，解題能力跟著下降。
  <https://aider.chat/2024/08/14/code-in-json.html>
- **Tam et al., "Let Me Speak Freely?"（EMNLP 2024 Industry）**：格式限制越嚴，推理任務掉得越多。先自由作答、再轉成指定格式（NL-to-Format），幾乎能把損失補回來。
  <https://arxiv.org/abs/2408.02442>

**對 SOM 的意義**：這是 L3 最便宜、最直接的修法。record 的 schema 維持不變，`records.py` 與組裝器都不用動，只改模型讀寫的**序列化格式**：

- op 的標頭仍寫成一行 JSON。
- `INSERT_BLOCK` 的 `source` 改放在緊接的 fenced code block 裡，原樣輸出，不做跳脫。

由一個 serializer／parser 負責兩種格式互轉，並用 round-trip 測試保證 bytes 相同。003 的失敗類別 1 應該會直接消失。這兩篇的數據來自大模型的 prompting，我們是 1.5B 的 SFT，但跳脫錯誤在 eval-001、003 都實際看到了，所以方向成立。

### 2. Constrained decoding：只約束結構，不約束程式碼（對應失敗類別 1、3）

- **XGrammar（Dong et al., 2024）**：CFG／JSON schema 約束解碼，額外開銷接近零，可保證輸出 100% 符合結構。
  <https://arxiv.org/abs/2411.15100>
- **Monitor-Guided Decoding（Agrawal et al., NeurIPS 2023）**：用靜態分析決定「此處只能出現哪些識別字」。SantaCoder-1.1B 加上 MGD 後，編譯率勝過 text-davinci-003。重點在於小模型最缺的是全域資訊，而不是語法。
  <https://arxiv.org/abs/2306.10763>
- **SynCode（Ugare et al., TMLR 2025）**：用 Python／Go 的 grammar mask，平均消除 96% 的語法錯誤。
  <https://arxiv.org/abs/2403.01632>
- **Type-Constrained Code Generation（Mündler et al., PLDI 2025）**：94% 的編譯錯誤是型別錯誤。加型別約束後，編譯錯誤減半以上，功能正確率相對提升 3.5–5.5%。
  <https://arxiv.org/abs/2504.09246>
- **The Alignment Problem in Constrained Code Generation（Biagiola et al., 2026-06）**：約束器只要**不完整**，也就是會拒絕合法程式，功能正確率最多會掉 97%，還常常超時。約束會把模型推進低機率的區域。
  <https://arxiv.org/abs/2606.21619>
- **Grammar-Aligned Decoding（Park et al., NeurIPS 2024）**：單純把 token mask 掉會扭曲模型的機率分布，他們的 ASAp 可以修正。對 greedy 解碼影響較小。
  <https://arxiv.org/abs/2405.21047>
- **CRANE（Banerjee et al., ICML 2025）**：自由生成與受約束生成交替進行，比全程約束和完全不約束都好，最多高 10 個百分點。
  <https://arxiv.org/abs/2502.09061>

**對 SOM 的意義**：約束只放在我們**能完整寫出規則**的地方：

- L3 的 op 標頭文法：op 名稱、欄位、`path`。
- block 名稱只能取自 L2 topology 宣告的清單，這相當於 MGD 對「全域識別字」的做法，可以直接消除失敗類別 3。
- snippet id 只能取自 ISA。

**不要**用 Python grammar 約束 `source` 內容。Python 文法不完整的風險正是 Biagiola et al. 量到的 97% 下降來源，而且 CRANE 也顯示把程式碼留給模型自由寫比較好。語法正確性交給組裝後的 `ast.parse` 與 fixture 判定。另外要注意，mlx-lm 沒有內建 XGrammar，要自己用 `logits_processors` 接 token mask，或者先做最低限度的版本：每條 op 生成後就驗證，錯了立即停止。

### 3. Greedy 重複是已知問題（對應失敗類別 2）

- **Holtzman et al., "The Curious Case of Neural Text Degeneration"（ICLR 2020）**：greedy／beam 解碼會退化成重複，GPT-3 175B 也一樣，跟模型大小無關。
- **Welleck et al., Unlikelihood Training（ICLR 2020）**：在訓練時懲罰重複 token，從源頭降低重複。
  <https://arxiv.org/abs/1908.04319>
- **Li et al., "Repetition In Repetition Out"（2023）**：訓練資料本身的重複程度是退化的主因之一。
  <https://arxiv.org/abs/2310.10226>

**對 SOM 的意義**：

- penalty 的 context 只有 20 個 token，擋不住整段 block 的重複。可以把 context 拉到涵蓋一個 block，約 256；但 penalty 太強會壓到程式裡本來就該重複的識別字，需要實測。
- 比調參更根本的做法，是把序列縮短（見第 4 節）：一次只生成一個 block，就不會出現跨 block 的迴圈。
- 還要檢查 gold ops 本身是否有大量重複的 import／snippet 行。如果有，就是 Li et al. 指出的資料端成因。

### 4. 分層與分塊生成（對應失敗類別 2、4，並佐證累積式 conditioning）

- **Self-planning Code Generation（Jiang et al., TOSEM 2024）**：先規劃再實作，實作階段的輸入是「intent + plan」，而不是只有 plan。這與 003 改成累積式 conditioning 的結論一致。
  <https://arxiv.org/abs/2303.06689>
- **Parsel（Zelikman et al., NeurIPS 2023）**：把任務拆成階層式的函式描述，**每個函式分開生成**，再用測試搜尋組合。APPS 通過率比直接取樣高 75% 以上，需要的樣本數還更少。
  <https://arxiv.org/abs/2212.10561>
- **CodeChain（Le et al., ICLR 2024）**：引導模型寫模組化程式，並迭代自我修訂，在 APPS 與 CodeContests 上有提升。
  <https://arxiv.org/abs/2310.08992>
- **Diffs vs. Whole Files（2026-09，Qwen2.5-Coder-0.5B 微調）**：在小模型上，直接生成整份檔案在所有指標上都勝過 diff 式編輯。diff 只適合短小、集中在局部的修改。
  <https://arxiv.org/abs/2609.05779>

**對 SOM 的意義**：L2 topology 已經是 Parsel 那種「函式清單」。L3 可以改成**逐 block 生成**：

- 每次呼叫的輸入是 caption、plan、topology，加上已生成的 block，輸出只有一個 block 的 source，或一條 `INSERT_SNIPPET`。
- import 最後一次補齊，或由組裝器從已宣告的名稱推導。
- 單次輸出從 4096 token 降到約 500，重複與截斷都會少很多。
- 訓練 pair 的數量也從每個 family 1 筆變成平均 4 筆（400 個 block ÷ 101 個 family），等於用同一批資料多出幾倍的監督訊號。

Parsel 的「用測試搜尋組合」牽涉到推論時從候選中挑選，這跟 SOM 的 non-goal `candidate-selection-at-inference` 衝突，這一部分**不採用**。L3 維持 whole-block 生成，不改成 diff op。

### 5. 容量與資料量

- **Qwen2.5-Coder Technical Report（Hui et al., 2024）**：1.5B 與 7B 之間在各項 benchmark 上都有明顯差距，1.5B base 在 HumanEval／MBPP 的平均約 47%。
  <https://arxiv.org/abs/2409.12186>
- **LoRA Learns Less and Forgets Less（Biderman et al., TMLR 2024）**：在指令微調上，rank 夠高時 LoRA 可以追平全參數微調；在 continued pretraining 上則一直落後。我們的任務屬於學新的輸出格式，比較接近指令微調，所以 rank 16 可能偏低，只掛 top 16 層也可能不夠。
  <https://arxiv.org/abs/2405.09673>
- **Magicoder / OSS-Instruct（Wei et al., ICML 2024）**：用真實程式片段合成 75K 筆指令資料，7B 以下的模型就能逼近大模型。我們只有 81 個訓練 family，這個量級離「能學會寫新程式」還很遠。
  <https://arxiv.org/abs/2312.02120>

**對 SOM 的意義**：

- 在 L3 的格式與序列長度修好之前，先不要換 7B。否則無法分辨進步是容量帶來的，還是格式帶來的。
- 格式修好後，做一次 7B 對照，其餘設定完全相同。
- 擴語料的成本最高：每個 family 都要 gold、fixture、plan，還要過 curation gate。這一步排在最後，而且要先有「L3｜gold」的數字證明模型已經不是瓶頸在格式上。

## 建議實驗順序

每一步都用同一組 held-out 20 個 family 量，一次只改一個變數。主指標是 `som eval --teacher-forced` 的 **L3｜gold 到 fixture 數**（003：8/20）與 **fixture 通過數**（003：0/18）。

| 順序 | 改動 | 預期會消掉的失敗類別 | 成本 |
|---|---|---|---|
| 1 | L3 序列化：op 標頭用 JSON，source 放 fenced block；serializer 要通過 round-trip 測試 | 1 | 低：只改 `train.py` 的 target 與 `evaluate.py` 的 parser |
| 2 | 逐 block 生成 L3 | 2、截斷 | 中：pair 定義、chain 迴圈、import 的處理方式 |
| 3 | 結構約束：block 名稱 enum 取自 topology，op 標頭加文法 | 3 | 中：mlx `logits_processors` 要接 token mask |
| 4 | LoRA 掛全部層、rank 提高到 32–64；再做 7B 對照 | 4 | 中高：訓練時間 |
| 5 | 擴語料 | 4 | 高 |

**不做**：

- 用 Python grammar 約束 `source`，原因見第 2 節。
- 推論時抽多個候選再用 fixture 挑，原因是 non-goal `candidate-selection-at-inference`。
- 把 L3 改成 diff op。

## 結果

### 實驗 A：`layer-sft-004`（L3 改用 fenced 格式）

只改 L3 的序列化格式（`src/som_core/wire.py`），其餘設定與 003 相同。最長的 pair 是 2896 token；`best_step` 為 250，也就是最後一步，l3 的 val loss 仍在下降。

| 量測 | 003 | 004 |
|---|---|---|
| chain 到得了 fixture | 7/20 | 14/20 |
| chain fixture 通過 | 0/18 | 0/18 |
| L2｜gold 合法 | 20/20 | 20/20 |
| L3｜gold 解析失敗（stage `l3`） | 12 | 4 |
| L3｜gold 組裝失敗（stage `assemble`） | 0 | 7 |
| L3｜gold 到得了 fixture | 8/20 | 9/20 |
| L3｜gold fixture 通過 | 0/18 | 0/18 |

L3｜gold 的失敗拆開來看：

- **解析失敗 4 筆**：3 筆是 code block 沒收尾。例如 `99-langgraph` 的輸出長 19,462 字元，同一句檢查被一再重複，直到撞上 4096 token 上限。另外 1 筆是一行裡塞了兩條 op。
- **組裝失敗 7 筆**：5 筆是 block 沒照 topology 的順序插入，例如先寫主類別，才寫它依賴的 error 類別；1 筆引用了未宣告的 block；1 筆在 `CREATE_FILE` 裡多加了 L1 的欄位。
- **fixture 失敗 9 筆**：品質 gate 報出的主要問題是 F821（名稱未定義）。`29-stdlib-os-atomic-file-write` 只 import 了 `json`，程式卻用到 `os`、`tempfile`，block 名稱也從 `AtomicFileOps` 被改成 `AtomicFileOperations`。

**判定**：格式錯誤從 12 筆降到 4 筆，剩下的都是重複到 token 上限；chain 到得了 fixture 的數量翻倍。主指標只從 8 升到 9，因為原本卡在解析的 family，現在改卡在「順序／引用」與「漏 import」這兩個問題上。實驗 B 用 topology 指定下一個 `NEXT` block，可以直接排除順序錯誤與未宣告 block；把 import 拆成最後一次呼叫生成，則會讓漏 import 的問題更容易看清楚。

### 實驗 B：`layer-sft-005`（逐 block 生成 L3，250 步）

每個 block 產生一筆 `l3` pair，prompt 帶 `DONE:` 與 `NEXT:`；每個 family 另有一筆 `l3i` pair，負責 import。train pair 數從 243 增加到 581。`best_step` 為 250，也就是最後一步，l3 與 l3i 的 val loss 都還在下降。

| 量測 | 004 | 005 |
|---|---|---|
| chain 到得了 fixture | 14/20 | 6/20 |
| L2｜gold 合法 | 20/20 | 16/20 |
| L3｜gold `l3`／`l3i`／`assemble` | 4 / – / 7 | 8 / 3 / 1 |
| L3｜gold 組裝成功（fixture＋no_fixture） | 9 | 8 |
| fixture 通過 | 0/18 | 0/18 |

組裝失敗從 7 筆降到 1 筆，代表順序錯誤確實消失了；但新的失敗集中在 block 回覆的格式上：

- **code block 沒收尾（5 筆）**：其中 3 筆的 source 已經寫完（267–407 token），模型卻直接結束，沒有寫收尾的 ```` ``` ````；1 筆把 ```` ``` ```` 黏在最後一行程式後面；1 筆重複到撞上 1024 token 上限。
- **抄錯 block 名稱（2 筆）**：例如 `RedisPubSUBManager`。`NEXT` 已經指定了名稱，要模型再抄一次，反而多了一個出錯的機會。
- **`INSERT_BLOCK` 誤用 snippet 的欄位（2 筆）**。
- **`l3i` 重複同一行 import，直到撞上 512 token 上限（3 筆）**。

L2｜gold 從 20/20 降到 16/20。原因可能是 l1、l2 在 pair 總數中的比例從 1/3 降到約 1/7，250 步內分到的更新次數變少了。

**修正（`blockwise-open`，並改跑 500 步，記為 `layer-sft-005b`）**：

- block 回覆的標頭不再帶 `path`／`block`，由 `NEXT` 決定。
- `INSERT_BLOCK` 的 source 一路寫到回覆結束，不再需要收尾的 fence。
- 回覆一旦撞上 token 上限，就記為 `l3`／`l3i` 失敗，不讓被截斷的 source 流進 fixture。

這次一次改了兩個變數（回覆格式與步數）。005b 的結果不能只歸因於其中一項。

### 實驗 B'：`layer-sft-005b`（`blockwise-open`，500 步）

pair 與 005 相同（train 581 筆）。`best_step` 為 500，也就是最後一步，四層 val loss 都還在下降。前兩次重跑都被 host 的記憶體壓力回收機制中途砍掉；加上 `mx.set_cache_limit(8 GiB)` 之後跑完，MLX peak 停在 14.5 GiB（`memory.jsonl`：active 約 1.0 GiB，cache 頂在 8 GiB）。

| 量測 | 004 | 005 | 005b |
|---|---|---|---|
| chain 到得了 fixture | 14/20 | 6/20 | 12/20 |
| L2｜gold 合法 | 20/20 | 16/20 | 18/20 |
| L3｜gold `l3`／`l3i`／`assemble` | 4 / – / 7 | 8 / 3 / 1 | 9 / 0 / 1 |
| L3｜gold 組裝成功（fixture＋no_fixture） | 9 | 8 | 10 |
| fixture 通過 | 0/18 | 0/18 | 0/18 |

- **`l3` 失敗 9 筆，全部是逐行重複撞上 1024 token 上限**：最後 40 行裡，同一行出現 8–28 次（例如 `43-pandas` 的 `score_mask = np.where(...)`、`14-pydantic` 的 regex 字串）。重複的單位是一整行，長度超過 repetition penalty 的 20 token 視窗，所以懲罰擋不住。格式錯誤、抄錯名稱、順序錯誤都已經歸零。
- **`l3i` 失敗歸零**，但到 fixture 的 9 筆中有 3 筆仍漏 import：`62-aiohttp` 只 import 了 `asyncio`、`typing`，程式卻用到 `aiohttp`、`json`；`65-lxml`、`77-jwt` 也一樣。
- **其餘 6 筆 fixture 失敗是 block source 本身的語法錯誤**，例如 `29-stdlib` 寫出 `raise from exc`，`54-pyyaml` 的 `try` 沒有 `except`，函式裡還用到 `self`。
- 組裝失敗 1 筆：`01-fastapi` 給 `sqlalchemy_declarative_base` 多傳了模板沒有的 `name` 參數。

**判定**：`blockwise-open` 消掉了 005 的格式類失敗；chain 到得了 fixture 從 6 回到 12，主指標 10 是目前最高，但 fixture 通過仍是 0/18。剩下的失敗依序是：逐行重複（9）、語法錯誤（6）、漏 import（3）。其中逐行重複屬於解碼問題，不需要重新訓練：先只放大 `repetition_context_size`（或改用 line-level no-repeat）重跑 005b 的 eval，可以跟實驗 C 的容量效果分開量測。

### 解碼修正：005b 加 no-repeat 80-gram（`eval-005b-nrng80`，不重新訓練）

`evaluate.py` 新增 `NO_REPEAT_NGRAM = 80`：禁止回覆中再次完成已經出現過的 80-token 片段。語料 gold 回覆裡最長的重複片段是 78 token（`70-selenium`），所以 gold 不受影響。

| 量測 | 005b | 005b＋nrng80 |
|---|---|---|
| chain 到得了 fixture | 12/20 | 13/20 |
| L2｜gold 合法 | 18/20 | 18/20 |
| L3｜gold `l3`／`assemble` | 9 / 1 | 7 / 1 |
| L3｜gold 到 fixture | 9 | 11 |
| fixture 通過 | 0/18 | 0/18 |

剩下 7 筆 `l3` 失敗仍然是撞到 1024 token 上限：重複的每行略有不同，不會形成完全相同的 80-gram。到 fixture 的 11 筆失敗原因：語法錯誤 7 筆、`NameError` 漏 import 2 筆（62、65）、斷言失敗 1 筆（77）。

### 天花板探測：7B 基座直接依 caption 寫 candidate（不經 SOM 層、不訓練）

探測腳本放在 session scratchpad（`direct_probe.py`）。模型是 `Qwen2.5-Coder-7B-Instruct-4bit`，解碼設定與 eval 相同，同一組 held-out 20 個 family。

- 結果：**0/18**。
- 程式幾乎都能跑。多數 family 是部分斷言通過、部分失敗，例如 14 通過 6 失敗 2、29 通過 5 失敗 1、48 通過 2 失敗 5。
- 29 唯一的失敗是傳給 `NamedTemporaryFile` 的 `dir=` 用了 `str`，fixture 要求 `Path`；caption 寫的是 "dir set to the target's parent directory"。
- 可見 fixture 逐條檢查 caption 寫到的細節，一個細節偏離整題就算失敗。

**判定**：7B 在沒有任何訓練、直接照 caption 寫的條件下，held-out 仍是 0/18。1.5B 加 LoRA、只用 81 個 family 訓練，要把 held-out 推到 18/18，只調容量或解碼不夠。這已經是模型規模與語料量的問題。

### 兩段式：旗艦模型只透過 SOM MCP 產生程式

改走兩段式架構。第一段是旗艦模型（`claude -p --model opus`）。它看不到檔案、也不能用其他工具，只能呼叫一個 MCP 工具：`som_compile(code, exports, doc)`，實作在 `src/som_core/mcp_server.py`。

第二段是 `src/som_core/dsl.py`，負責把 DSL 編譯成程式。DSL 是精簡過的 Python：不寫 import、不管排版、允許單行 compound。編譯器依序做以下幾件事：

1. 用 near-miss AST 規則擋掉已知寫法，退件時附一行理由，例如 unsafe yaml loader、`jwt.decode` 沒帶 algorithms、XXE、ECB、PBKDF2 次數不足、在 secret 上用 `==`、mutable default、bare except。
2. 自動補 import：只查非 holdout 語料的 import 表，查不到再到已 import 的套件上探測屬性。
3. 把 `exports` 保證成 top-level 名稱；re-export 的名稱會補上 `__all__`。
4. 在 corpus 的 quality config 下跑 `ruff check --fix` 和 `ruff format`。

模型的輸入是 caption 加上一份介面契約。契約從 fixture 的 AST 機械抽出，只列名稱與值，不含測試本體，內容包括：exports、monkeypatch 目標、patched callable 從 kwargs 讀的參數以及測試對這些參數值的斷言、raises 與 match 字串、用到的屬性、keyword 與字串鍵、被斷言的字串值。

評測對象是 holdout 20 題，其中 18 題有 fixture。

| 輪 | 改動 | fixture | 失敗 | 輸出 token（20 題） | 成本 |
|---|---|---|---|---|---|
| r1 | 只給 caption | 8/18 | 94 48 77 55 73 41 78 65 72 99 | 27928 | $1.26 |
| r2 | 加上介面契約 | 17/18 | 29 | 22240 | $1.13 |
| r3 | 契約加入「patched callable 讀的 kwargs」 | 17/18 | 65 | 21459 | $1.10 |
| r4–r6 | `exports` 參數必填、`__all__` re-export、套件屬性探測 | 17/18 | 43 | 22–23k | $1.15 |
| r7 | 同上 | 17/18 | 29（抽樣波動） | 20362 | $1.08 |
| r8 | 契約加入「測試對 kwargs 值的斷言」（`dir == target.parent`） | **18/18** | — | 21058 | $1.09 |

說明：

- 每題平均約 1k 輸出 token、2 turn，成本約 $0.05。第二段不經過旗艦模型，不耗它的 token。
- r1 失敗的根因是 fixture 檢查的名稱和訊息 caption 沒寫到，模型只能猜，與模型能力無關。從 r2 起，每個失敗都對應到契約少抽了一類事實，補上抽取規則就解決，沒有任何一條規則是針對單一 family 寫的。
- 29 的 Path 被轉成 str，跟 7B 探測時是同一種失敗。r8 以前這是抽樣波動（r1、r3–r6 通過）；r8 把斷言值寫進契約，才讓答案不必靠猜。
- 仍有 quality findings（mypy strict／pylint，每題 4–22 條）。fixture 目標不包含 quality gate，所以這裡不列入判準。
- 腳本 `mcp_eval.py`、`contract.py`、`why.py` 放在 session scratchpad。

### 第三段：旗艦只寫 SOM prompt，自訓模型翻成 DSL

架構改成跟生圖模型一樣分兩段：旗艦模型只負責寫 SOM prompt，自訓模型把 prompt 翻成 DSL，最後照舊交給 `compile_dsl` 產生程式。

- **prompt 語言**：`src/som_core/prompt.py`，格式是 `tag: value` 行，標籤依序為 intent、exports、calls、patch、kwargs、asserts、raises、attrs、keywords、keys、values、avoid。
- **推論迴圈**：`src/som_core/generate.py`，最多重試 3 次，推論時不讀 fixture，也不做 best-of-N。觸發重試的條件有兩種：
  - `compile_dsl` 退件；
  - 通過編譯，但 conformance 檢查只拿 prompt 本身比對、發現沒做到。檢查三件事：intent 給的預設數字、intent 裡 `Name(k=literal)` 形式的預設參數、`kwargs` 要用 keyword 傳給被 patch 的函式。
- **MCP 入口**：`som_generate(prompt)`。
- **訓練資料**：completion 是 `to_dsl(gold)`，所有 in-scope family 的 round-trip gate 都綠。prompt 來源有三種：
  - 機械版：caption 加上從 fixture 抽出的契約；
  - 旗艦版：`claude -p --model opus` 每個 family 寫 3 份；
  - 條件 dropout：隨機拿掉部分非必要標籤的副本。

**機械 prompt 評測（holdout 20 題，有 fixture 的 17–18 題）**

「編譯」欄是 20 題中 `compile_dsl` 至少接受一次的題數；fixture 欄的分母是有編譯成功、而且有 fixture 的題數。

| 輪 | 模型 | 訓練 | 編譯 | fixture |
|---|---|---|---|---|
| P0 | Qwen2.5-Coder-1.5B | 無 | 10/20 | — |
| P0 | Qwen2.5-Coder-7B | 無 | 19/20 | 2/17 |
| P0 | Qwen3-Coder-30B-A3B | 無 | 17/20 → 16/20（加 `calls` 標籤） | 1/15 → 5/14 |
| som-dsl-002 | 7B | 旗艦版＋機械版＋dropout，2040 pairs，lr 1e-4，450 步 | 15/20 | — |
| som-dsl-003 | 30B | 機械版 255 pairs，lr 1e-4，best 250 步 | 8/20 | 0/8 |
| som-dsl-004 | 30B | 機械版 255 pairs，**lr 1e-5**，best 200 步 | 18/20 | 9/17 |
| som-dsl-005 | 30B | 旗艦版（缺的介面標籤從契約補上）＋機械版＋dropout，2040 pairs，lr 1e-5，best 150 步（val 0.392） | — | — |

- 30B MoE 在 scale 20 下用 lr 1e-4 會壞掉，語法錯誤佔多數；降到 1e-5 之後，val loss 從 0.79 降到 0.35。
- 004 在機械 prompt 上的失敗分成三類：
  - 邏輯錯：14、48、99；
  - raises 訊息被改寫：72、78；
  - 缺名稱：10 缺 `app`，62 用了 `ResponseInfo`，這個名稱在 aiohttp 不存在。

**端到端評測**：旗艦模型是 Opus，只開放 `som_generate`，輸入是 caption 加契約，SOM 模型用 som-dsl-004。

| 輪 | 旗艦 effort | fixture | 失敗 | 旗艦輸出 token（20 題） | 成本 |
|---|---|---|---|---|---|
| e2e-004 | 預設 | 13/18 | 14 40 55 62 65 | 16213 | $0.97 |
| e2e-004-low | low | 10/18 | 01 14 40 48 54 55 73 94 | 13142 | $0.87 |
| e2e-005-low | low（SOM 模型 som-dsl-005） | 13/18 | 14 40 55 62 73 | 12256 | $0.87 |
| e2e-006-low | low；SPEC 限 intent ≤500 字元、不重述其他標籤 | 9/18 | 01 14 31 40 48 73 78 94 99 | 10843 | $0.82 |

- **失敗原因**：兩輪的旗艦 prompt 都是對的，失敗全出在 SOM 模型，以下是 e2e-004 各題：
  - 62：該有預設值的參數被寫成必填，新的 signature conformance 檢查能擋下來；
  - 65：把同一個 keyword 傳了兩次（`parser=p, **{"parser": p}`）；
  - 55：缺 `_env_constructor` 屬性；
  - 40：`WatchError` 的用法與 lambda 參數個數寫錯；
  - 14：raw string 的開頭引號被吞掉，見下方。
- **14 的 raw string 引號**：prompt 寫 `r'^\+…$'` 時，模型會輸出 `re.compile(r^\+…$)`，base 和 adapter 都這樣。把 prompt 改成雙引號或反引號之後就正常。修法是 `dsl_messages` 把單引號字面值轉成雙引號（`double_quote`），訓練和推論都走這條路徑。
- **token**：effort low 時 thinking 是 0，旗艦的輸出 token 幾乎全部是 prompt 本身。
  - 20 題的 prompt 共 2.6 萬字元，其中 intent 佔 65%。
  - regex 和程式碼這類文字平均約 2 字元一個 token。
  - 要達到 ≤10k token，每題 prompt 大約要壓到 1000 字元以下。
- **輸出變異**：兩輪之間 fixture 的通過名單差異很大，62、65 在 low 那輪反而通過。代表 SOM 模型對 prompt 措辭很敏感，下一輪要拿旗艦寫的 prompt 來訓練。
- **som-dsl-005**：val 在 150 步最低（0.392），之後一路升到 900 步的 0.451，150 步之後就是過擬合。
- **e2e-005-low 失敗原因**：旗艦 prompt 依然都對，失敗全出在 SOM 模型：
  - 14：SemVer regex 括號沒配對，`re.error: missing ), unterminated subpattern`；
  - 40：沒有 catch `WatchError` 重試、`InMemoryPipeline` 沒有 `close`、lambda 少了 `data` 參數；
  - 55：用了不存在的 `yaml.NonScalarNode`；
  - 62：`CancelledError` 沒有往外拋，重試 4 次後丟 `FetchError`；
  - 73：IV 傳成 None；raises 訊息被改寫成 `Key must be exactly 16 or 32 bytes`。
- **e2e-006-low**：SPEC 要求 intent 最多 500 字元、不重述其他標籤已帶的內容，token 從 12256 降到 10843；抽查失敗題，旗艦 prompt 仍然正確，是 SOM 模型漏做 prompt 明寫的子句（14 的 `return v.lower()`、31 的 TypeError 檢查）。跑到一半時編譯器加了兩項檢查：套件屬性探測、同名先用後定義的退件，所以後段題目用的是新編譯器。
  - 01 在舊編譯器下失敗：`engine` 在定義之前就被使用，auto-import 卻把它解析成 `from sqlalchemy import engine`；現在改成退件並要求把定義上移。
  - 套件屬性探測：`import yaml` 之後讀 `yaml.NonScalarNode`，這種不存在的屬性在編譯期退件；gold round-trip gate 沒有誤退。
  - raises 訊息檢查量過 gold：359 條有 91 條找不到字面訊息（f-string、測試自己丟的例外），誤退太多，不做。
  - 01 的退件之後改成機械上移（`_hoist`）：名稱在頂層被讀取、而且它是後面某個單一目標的頂層 `Assign`，同時右邊沒有讀到中間區段綁定的名稱時，就把整段賦值搬到使用點之前。class 繼承順序顛倒這類搬不動的情況，仍然退件。
- **自我檢查（不採用）**：讓 SOM 模型逐條對照 prompt 子句檢查自己的輸出。14 個失敗候選全部被標出，但 22 個通過的候選也有 14 個被標，雜訊太大。
- **replay**：把兩輪 e2e 存下的最後一份旗艦 prompt 重新生成，並用 fixture 判定，不再付旗艦成本。兩次都用加 `_hoist` 之前的編譯器。

  | base | adapter | 通過 | 失敗 |
  |---|---|---|---|
  | 4bit | som-dsl-005 | 23/35 | 005：14 40 62 73；006：01（沒編譯過）14 31 40 48 73 78 94 99 |
  | 8bit | som-dsl-005（在 4bit 上訓練） | 26/36 | 005：40 73 77 78 99；006：01 48 73 78 94 |

  adapter 沒有重新訓練，只把推論的 base 換成 8bit，通過數就多了 3 題；主要差在 006 那組 prompt（14、31、40、99 轉為通過）。下一步是直接在 8bit base 上訓練（som-dsl-006）。
- **8bit replay 失敗分析**：48、73、78、94 的旗艦 prompt 都正確，錯在 SOM 模型沒照做。
  - 48、78：自己改寫了 raises 訊息。
  - 94：方法少了 intent 寫明的預設值 `model='claude-sonnet-5'`。
  - 73：用了不存在的 `algorithms.AESGCM`。
- 據此加了三項檢查，都只在重試時觸發，不改變 gold 的編譯結果：
  - **raises 訊息 conformance**：模組自己用字串 raise 了同型別的例外、而且 match 片段含空白時，要求有字串能產生這段訊息；f-string 的欄位和 `%s`、`{}` 當萬用字元。用 849 份旗艦 prompt 對 gold 量，15 份被標，分屬 5 個 family，訊息都來自測試的 fake 或 Enum 自帶的措辭，測試裡逐一列名容許。能抓到 48、78。
  - **方法預設值**：簽名檢查擴到 export class 的公開方法；`subprocess.run(check=True)` 這類帶點號的呼叫不算。gold 被標 5/849。能抓到 94。
  - **屬性探測擴到 from-import**：`from pkg import sub` 而 `pkg.sub` 是模組時，一樣逐層探測（`algorithms.AESGCM` 會被退件）。能抓到 73。
- **SPEC**：e2e-006 的 intent 平均 73 字、692 字元，超出 500 字元上限。改成「at most 60 words」，因為字數比字元數好估。
- **som-dsl-006**：直接在 8bit base 上訓練，資料同 005（2040 pairs），lr 1e-5，300 步。val 從 step 0 的 0.951 降到 250 步的 0.336（best），300 步回升到 0.352。
- **replay（8bit＋som-dsl-006）**：26/36，和 8bit＋005 同分，失敗名單換了一批：005 那組是 01 14 40 73 99，006 那組是 14 48 55 73 99。只換 adapter 追不到 18/18。
- **識別字出現檢查（不採用）**：intent 裡看起來像程式碼的識別字（含點號、camelCase、底線）都要在模組中出現。拿 849 份旗艦 prompt 對 gold 量，第一版誤標 359 份，收窄後仍誤標 101 份，雜訊太大。
- **把模組回給旗艦（e2e-007-low）**：`som_generate` 改成回傳生成的模組，附上 conformance 沒過的點（`unmet:`），讓旗艦自己判斷要不要改 prompt 重送。fixture **18/18**，14 和 73 在 replay 會失敗，這輪都通過了。代價是 token 漲到 30295，成本 $1.74。
  - 40 佔 5967 token：送了 8 次，其中 5 次是同一個編譯錯誤，`redis.WatchError` 沒有被 auto-import。
  - 每次重送都是整份 prompt，而且最後的回覆會附上一段說明。
- **協定調整（e2e-008）**：
  - 修改只送 `fix: <點>` 行，`prompt.revise` 會把它併進上一份 prompt 的 intent。第一次就送 `fix:` 會退件。
  - 模組沒問題就只回 `done`。
  - tool description 加一句：只有模組和需求矛盾時才修改。
  - 編譯器補上裸套件 import：未解析的名稱如果是 symbol index 裡某個套件的根名（例如 `redis`），就補 `import redis`。gold round-trip gate 仍然全綠。
- **e2e-008-low**：fixture **17/18**，token **14441**（e2e-007 是 30295），成本 $1.39。
  - 40 只用了 814 token，裸套件 import 省掉了重複的編譯錯誤。
  - `fix:` 重送都是短而具體的真缺漏，例如 01 的 Query 預設值、14 的 regex、29 的 try/unlink。
  - 唯一的失敗是 48：第二處 raise 把訊息改寫成 `binary values in {0, 1}`。conformance 只要求至少有一處產生 match 片段，另一處 raise 已經符合，所以沒擋下來。
  - token 組成：首次 prompt 共 19917 字元，其中 intent 佔 12245，平均 64 字；`fix:` 重送共 4623 字元。整體約 1.7 字元一個 token。
- **改寫訊息檢查**：同型別 raise 的訊息裡，如果依序出現 match 片段的每個字、只是中間夾了一兩個字，就判定為改寫並要求照抄。docstring 不算；如果那段訊息本身是另一條 `raises` 的片段，也不算（例如 72 的 `Salt length must be at least` 和 `Salt must be at least`）。用 849 份旗艦 prompt 對 gold 量，誤標 0 份。
- **SPEC 再收緊（e2e-009）**：intent 上限從 60 字改成 40 字；修改最多一兩行 `fix:`，每行 ≤12 字。
  - 跑到一半，SPEC 又補了一句「comments and lint pragmas are dropped」，起因是 41 要求加 `# pylint: disable`，這種要求 DSL 做不到。
- **e2e-009-low**：fixture **18/18**，token **16185**，成本 $1.46。比 e2e-008 多，主要是變異：48 一題就送了 7 次、用 2129 token，每次重送都是真缺漏，但 SOM 模型修好一點就弄壞另一點。
  - 20 題共 48 次呼叫，只有 01、09、43 一次過，其餘幾乎每題都重送一次。重送大多是 SOM 模型沒照 intent 做，例如 55 的「無參數方法」、31 的「`@dataclass` 加 `default_factory`」。
  - 最後回覆：18 題是 `done`；29、41 附了說明，48、94 在 `done` 後面接了一段。
  - token 組成：首次 prompt 共 17728 字元。intent 佔 8714 字元，平均 46.7 字，SPEC 寫的 40 字上限沒被遵守，parser 也不擋。`calls` 3294、`raises` 1330、`exports` 1096、`attrs` 978、`keywords` 460、`values` 349、`keys` 317，這些全是從手上的契約照抄。`avoid` 1000。`fix:` 重送共 3227 字元。
  - 算底價：就算 20 題都一次過，首次 prompt 平均約 890 字元，加上每題固定開銷，也在 10k 上下。所以要同時做兩件事：不再抄契約，並且減少重送。
- **契約改傳路徑（e2e-010）**：prompt 新增 `contract: <path>` 標籤。`prompt.expand_contract` 在工作目錄讀那個檔（檔內是 `render_prompt(contract_fields(fixture))`，和旗艦原本拿到的契約文字相同），補上 prompt 沒寫的標籤；prompt 自己寫的標籤優先，和訓練資料 `flagship_prompts` 的合併方向一致。路徑跑出工作目錄就退件。評測把契約另存成 `cwd/contract.som`，並要求旗艦寫 `contract: contract.som`、不要抄它的標籤。旗艦拿到的資訊不變，只是不必重打。
  - 冒煙測試（01，單題）：契約展開正常。SOM 模型寫了 `class Item(Base)` 卻沒定義 `Base`，接著自己寫出 `from sqlalchemy import Base`，編譯器沒有擋。補上檢查：from-import 的名稱如果在已安裝套件裡找不到，就退件。gold round-trip gate 仍然全綠。
- **e2e-010 結果**：fixture 14/18，旗艦輸出 9,671 token（e2e-009 是 16,185），成本 $1.23。token 目標第一次達到，但通過率從 18/18 掉到 14/18。
  - 失敗的 14、29、40、55 是同一型：旗艦用 `fix:` 點出缺漏，SOM 模型不照做，旗艦接著回 `done`。
    - 14：`mode='before'` 一直沒加。
    - 29：沒用 `json.dumps`，也沒有 try/unlink。
    - 40：兩次 fix 都寫了「never .decode()」，三次生成都還是 `.decode()`。
    - 55：fix 寫了 `value.split(':',1)`，程式仍然只有 `os.environ.get(key, key)`。
  - 契約改傳路徑把首個 prompt 大約減半。剩下的 token 大多花在 intent 和 fix 上。
- **否決的檢查：intent 點名的 `module.attr` 必須出現在程式裡**。gold 被誤報 139/849，多半是 `collections.abc`，或 avoid 型的提及（`random.seed`）。
- **fix 點檢查（`generate.fix_misses`）**：只看旗艦的 `fix:` 行，不看整段 intent。規則有三條：
  - fix 行裡形如呼叫的名稱（`json.dumps(`），程式要呼叫它或定義它；
  - `never/no X()` 點名的呼叫不能出現；
  - `k='v'` 這類字面值要以 keyword、預設值或指派的形式出現。

  有沒兌現的點，SOM 端就重試（最多 3 次），不花旗艦 token。
  - 同樣的規則套到整段 intent，gold 誤報 104/849（intent 裡的 `n=2`、`status_code=400` 描述的是執行期的值），所以只套 fix 行。
  - 在 e2e-010 的最終候選上，只有 14、29、40、55 被抓到，正好是四題失敗。
  - 誤報的代價只是多重試，最後仍取未兌現項最少的那一版。
  - server 用 `_FIXES` 累積同一份 prompt 的 fix 點；換新 prompt 時清空。
- **Replay（`replay-010`）**：拿 e2e-010 的最後一份合併 prompt 加上 fix 點，重新走 `generate_module`，再跑 fixture。
  - 40 過了。
  - 14 補上了 `mode='before'`，但寫出 `re.sub(r"[ -()]", ...)`。`[ -(]` 是一段字元範圍，不是三個字元。
  - 29 三次 greedy 重試產出完全相同，而且都是 `NamedTemporaryFile(delete=False)`，沒有 try 包住，也沒有 unlink。fixture 真正失敗的是 `test_temp_file_cleaned_up_on_failure`。改用 temp 0.7 取樣，6 次只有 1 次兌現 fix。
  - 55 的最後一份 prompt 先因語法錯被退，後兩次都漏掉 export `load_custom_config`。退件訊息只說「undefined」，模型沒看懂。
- **新增兩條 near-miss 規則，並改寫一條退件訊息**：
  - `re.*` 的字元類別裡，兩端都是標點的 range 會被點名（`_punct_ranges`），gold 誤報 0。
  - 模組裡有保留的暫存檔（`delete=False` 或 `mkstemp`），卻從沒呼叫 unlink/remove，就退件。gold 有 2 個使用者，誤報 0。
  - exports 列了但沒定義的名稱，改報「exports lists X but the module never defines it」，不再混進 undefined 清單。
- **否決的檢查：`patch` 目標的末段名稱必須出現在程式裡**。gold 誤報 2/300，而且抓不到 29。
- **重試回饋合併**：原本 compile 一退件，`conformance` 和 `fix_misses` 就不跑，模型每輪只看到一類問題，改好 A 又漏 B。現在只要程式能 parse，退件訊息和未兌現點一次全報。
- **replay-012**：14、29、55 仍然 fixture 失敗。
  - 14：`mode='before'` 還是沒加。
  - 29：`tmp_path` 在 fsync 之後才指定，fsync 一失敗，except 裡的 `tmp_path` 還是 None，暫存檔留下。
  - 55：非 mapping 的 root 沒有退件。

  三題都是語意上的小錯。再照 holdout 的失敗逐題加規則，等於拿測試集調參，所以到此為止。上面兩條 near-miss 規則也有同樣的疑慮：它們來自 holdout 的失敗，只是 gold 誤報 0，而且描述的是通用錯誤。下一步看完整 e2e-011；若還差，就用訓練教 fix 兌現（som-dsl-007）。
- **e2e-011**（som-dsl-006 加上以上規則）：fixture **15/18**，旗艦輸出 **9,334 token**（r8 是 21k），$1.28。
  - token 目標已達；差距只剩通過率。
  - 仍失敗：14（4 failed）、29（1 failed）、99（2 failed）。40、55 轉綠。09、10 沒有 fixture，不在分母。
- **som-dsl-007：訓練模型兌現 fix**。e2e 失敗的共通點是：旗艦指出錯處，SOM 模型重寫時卻沒照改。
  - 新的 pair 型別 `dsl_fix`（`fix_pairs`）：prompt、near-miss 候選轉出的 DSL、`fix: <why_wrong>` 三者組成一次修訂，目標是 gold DSL。對話格式和推論時的 `revision_messages` 相同。
  - 非 holdout 的 283 個 row 產出 1,415 個 fix pair，token 長度 p50 1025、p95 1818；38 個超過 2048 會被截斷。
  - 推論端：server 保留上一輪選中的 DSL（`result["dsl"]`）。收到 `fix:` prompt 時，第一次生成改成修訂那份 DSL（`previous`），不再從頭寫；重試只帶最新一次嘗試。
  - 訓練設定沿用 006，只多 `--revisions`：`runs/som-dsl-007`，300 iters，lr 1e-5，caps 2048。
  - 訓練結果：val dsl loss 0.951（step 0）→ 0.369（50）→ 0.342（200）→ **0.3414（250，最佳，已存）** → 0.349（300）。006 的最佳是 0.336，純 DSL 翻譯幾乎沒變。val dsl_fix 全程約 0.0045。這個數字看不出東西，因為修訂目標大多和輸入的 DSL 相同，模型照抄就能拿到低 loss；fix 有沒有真的被照改，只能看 e2e。10,880 s，peak 50.8 GB。
  - 流程事故：9/26 00:10 左右機器重開，scratchpad 被清空，e2e-012 第一次跑到一半中斷。其中 7 題本來就已經因斷網（`ENOTFOUND`）失效。eval 腳本已從 transcript 重建。旗艦版 recap prompt（訓練 `--prompts-dir` 的來源）也一起遺失，下次訓練要重新產生，約 $17。
  - 第二次事故：重跑時用 JOBS=2，兩題同時冷載入 30B 模型（每份 31 GB，機器 64 GB），swap 被吃滿，兩題都卡滿 1800 s timeout。timeout 只殺了 claude，它底下的 mcp_server 成了孤兒，繼續佔著記憶體。eval 腳本因此改成 timeout 時殺整個 process group，預設 JOBS=1。
- **e2e-012**（som-dsl-007，EFFORT=low，JOBS=1）：fixture **14/18**，旗艦輸出 **8,981 token**，$1.12。
  - 翻轉：14 轉綠，靠兩輪 `fix:`；48、55 轉紅；29、99 仍紅，99 從 2 failed 降到 1 failed。
  - 45 次 `som_generate` 裡有 25 次修訂，全部是 `fix:`，沒有整份重送。所以 `previous` 修訂路徑有被用到，四題失敗都是模型沒照 fix 改。48 最明顯：conformance 兩次呼叫、共 6 次嘗試都抓到缺了 `'binary in'` 訊息，模型拿著上一輪的 DSL 仍然沒補上。
  - 分佈不一致：旗艦的 fix 行一行塞 3–5 個點（例：「add precision/recall/f1…; no imbalance raise; non-binary message…; roc_auc only with y_prob」），`dsl_fix` 訓練 pair 每次只有一個 `why_wrong`。
  - 判讀：和 e2e-011 的 15/18 相比，差距落在旗艦 prompt 本身的變異範圍內（每輪都有 ±1–2 題翻轉），007 在 e2e 上看不出淨增益。
  - calls.jsonl 記下的是經過 `revise` 合併、`expand_contract` 展開之後的 prompt，看不到旗艦的原文，也看不出旗艦有沒有照要求寫 `contract: contract.som`。
- **拆點重播**（`replay_split.py`，som-dsl-007，greedy）：把 e2e-012 的 `som_generate` 呼叫重播三種方式，檢驗「一行多點」是不是 fix 沒被照改的原因。A＝照 e2e 一次修訂送整行；B＝以「; 」拆開，一點一次修訂；C＝拆開後當多行 `fix:` 一次送。

  | family | A | B | C |
  |---|---|---|---|
  | 14（對照） | 通過 | 通過 | 通過 |
  | 48 | 5 failed | 5 failed | 5 failed |
  | 29 | 1 failed | 1 failed | 1 failed |
  | 55 | 4 failed | 2 failed | 1 failed |
  | 99 | 2 failed | 2 failed | 1 failed |

  - A 重現 e2e（14 綠、四題紅），重播可信。拆點沒有讓任何一題轉綠；C 讓 55、99 各少錯幾格，但不是主因。
  - 48：e2e 的 6 次嘗試產出逐位元組相同的 DSL。重試帶的是 `raises: no ValueError message contains 'binary in '`，模型原樣吐回上一版。
  - 29：fix「try must wrap write/flush/fsync/replace」有讓 DSL 變動，但 `try` 仍只包 `os.replace`；`fix_misses` 抓不到這種結構要求，unmet 顯示 0。
  - 根因：重試那一輪的 feedback（compile diagnostics 與 conformance 訊息）從沒出現在訓練裡。`fix_pairs` 只教 `fix: <why_wrong>` 一種措辭。e2e-012 全部 16 次重試中有 8 次和前一次逐位元組相同，只有 5 次把 diagnostics 清掉。
  - 可用資料：283 個非 holdout row 的 1,415 個 near miss 中，90 個（54 個 family）會被推論迴圈退件，其中 67 個是 `raises` 訊息，就是 48 的那種格式。
- **som-dsl-008：訓練模型讀懂自己的退件**。007 加 `--retries`，其餘設定不變。
  - 新 pair 型別 `dsl_retry`（`retry_pairs`）：prompt、一次嘗試、推論迴圈對那次嘗試的退件訊息三者組成一輪重試，目標是 gold DSL。退件訊息由 `generate.rejection` 產生，和 `generate_module` 送出的是同一個函式，措辭不會分岔。
  - 嘗試有兩個來源：near miss 轉出的 DSL，以及從 gold 機械敲掉一點的 knockout（`raises` 訊息插一個字改寫、`intent` 寫明的預設值改掉）。gold 編譯出任何 diagnostics 的 row 不出 pair；gold 本來就有的未兌現點會從 feedback 扣掉。
  - 數量：276 個 pair、138 個 family。第一條 feedback 的類別：`raises` 198、`intent` 65、compile 12、`kwargs` 1。
  - `--prompts-dir` 重建：007 用的 recap 在重開機時遺失。`recaption.py` 從 transcript 重建，SPEC 凍結在 9/24 的措辭，契約不帶 `calls`（`flagship_prompts` 會從 fixture 補上），所以 prompt 分佈和 007 相同，只是重新抽樣。重跑結果：283 個 family 各 3 份，共 849 份，格式退件 0，$11.51。
  - 訓練結果：train 3,564 pair（007 是 3,315；多出 249 個 retry），val 多 27 個 `dsl_retry`。val dsl loss 0.955（step 0）→ 0.369（50）→ 0.358（100）→ 0.344（150）→ 0.341（200）→ 0.357（250）→ **0.3384（300，最佳，已存）**，與 007 的 0.3414 持平。val `dsl_retry` 0.075 → 0.0014，和 `dsl_fix` 一樣是照抄就能拿到的低 loss，要看 e2e。300 步每步 1 pair，retry 約佔 7%，期望只看到約 20 個。5,254 s，peak 50.8 GB。
- **e2e-013**（som-dsl-008，EFFORT=low，JOBS=1）：fixture **14/18**，旗艦輸出 **8,305 token**，$1.17。和 e2e-012 同分。
  - 翻轉：29 轉綠（一次重試就清掉暫存檔 near-miss）；72 轉紅（2 failed）。48、55、99 仍紅，55 從 1 failed 變 4、99 從 1 變 4。
  - 重試：13 次重試中 8 次和前一次逐位元組相同，4 次清掉 diagnostics（e2e-012 是 16／8／5）。**008 沒有改掉照抄**。
  - 按 feedback 類別拆開：compile diagnostics（undefined 名稱、f-string 語法、不存在的屬性、暫存檔 near-miss）5 次重試全部有改，4 次清掉。`raises: no ValueError message contains …` 8 次重試全部逐位元組相同（48 ×4、72 ×2、78 ×2）。
  - 這 8 次都是真的漏：48 的 DSL 沒有任何含 `binary in` 的 raise，72 沒有 `below required`，78 沒有 `is forbidden in unencrypted JWT payload`。要補的是一整個帶條件的 raise 分支，不是改幾個字。
  - 判讀：`retry_pairs` 的 `raises` knockout 是在訊息裡插一個字，feedback 措辭和 e2e 一樣，但要做的修正只是刪掉那個字。模型學到的是局部改字，沒學到「缺整個 raise 就補一個」。另外 retry 在 300 步裡只出現約 20 次，而且 loss 是整份 DSL 的 token 平均，照抄的部分佔絕大多數，改動那幾行的梯度被稀釋。
- **som-dsl-009：knockout 改成刪掉整個 raise 分支**。008 設定不變，只在 `_knockouts` 多一種變體（`_dropped_raises`）。
  - 做法：用 AST 找出訊息含 `raises` 文字片段的 `raise`。如果它是某個 `if` 的唯一 body 而且沒有 else，就刪掉整個 `if`；如果它所在的 block 還有其他敘述，只刪這一行。刪除按行號從原文切掉，其餘位元組不變，確保 retry 的 target 和 attempt 只差那一個分支。刪完仍能 parse 的才保留。
  - 用意：e2e-013 那 8 次照抄，缺的都是整段帶條件的 raise。要讓訓練裡出現同樣的 feedback 措辭（`raises: no <Exc> message contains …`）配上「補回整個分支」的修正。
  - 數量：337 個 pair、138 個 family（008 是 276）。`raises` 類 259 個，其中 101 個是刪掉 raise 的形狀（含原本就漏掉 raise 的 near miss，被 `dict.fromkeys` 去重），158 個是改字；`intent` 65、compile 12、`kwargs` 1 不變。
  - 沒動的變數：retry 的曝光次數仍然約 20 次，loss 也沒加權。如果 009 的 `raises` 重試還是照抄，下一個單變數實驗再處理這兩點。
  - 訓練結果：train 3,619 pair（比 008 多 55），val `dsl_retry` 33（多 6）。val dsl loss 0.955（0）→ 0.386（50）→ 0.365（100）→ 0.348（150）→ 0.338（200）→ 0.340（250）→ **0.3373（300，最佳，已存）**，與 008 的 0.3384 持平。val `dsl_retry` 0.081 → 0.0025。6,188 s，peak 50.8 GB。
- **e2e-014**（som-dsl-009，EFFORT=low，JOBS=1）：fixture **13/17**（14 沒產出 `candidate.py`，實際算 13/18），旗艦輸出 **9,685 token**，$1.19。
  - 翻轉：55、72 轉綠；14、78、94 轉紅。48、99 仍紅。旗艦每輪寫的 prompt 不同，±1–2 題在雜訊範圍內，重試行為才是乾淨的訊號。
  - 14：greedy 退化，`ConfigDict(` 裡一直生成 `validate_by_*_schema-field_…=True`，直到 token 上限，括號沒關。每次重複都不完全相同，所以 80-gram 防護攔不到。12 次嘗試都是這樣。
  - 48：這次有寫出非二元的 raise 分支，但訊息是 `y_true must contain only 0 and 1`；fixture 從 5 failed 降到 1 failed。78：訊息寫成 `f'Sensitive claim "{key}" is forbidden …'`，引號和契約的 `'password'` 不同。兩題要的修正都是「換掉整句訊息」，不是插字，也不是補分支。
  - 重試統計（`retry_stats.py`，依觸發重試的 feedback 類別分）：

    | run | adapter | 重試 | 照抄 | 有改 | 清掉 | `raises` 照抄 | compile 照抄 |
    |---|---|---|---|---|---|---|---|
    | e2e-012 | 007（無 retry pair） | 16 | 8 | 3 | 5 | 4/8 | 2/5 |
    | e2e-013 | 008（retry，插字 knockout） | 13 | 8 | 1 | 4 | 8/8 | 0/5 |
    | e2e-014 | 009（再加刪分支 knockout） | 27 | 21 | 5 | 1 | 10/10 | 9/15 |

  - 同一題、同一句 feedback 的對照：72 的 `raises: no ValueError message contains 'below required'`，在 007 兩次重試都有改，在 008 兩次都照抄。
  - 判讀：retry pair 沒教會回應 feedback，反而強化了照抄。knockout 的 attempt 和 target 只差一兩行，模型在這類 pair 上學到的最低 loss 策略就是整份複製；009 多出 61 個幾乎相同的 pair，照抄率跟著上升（compile 類也從 0/5 升到 9/15）。再調 knockout 形狀或加曝光，都還是在同一種「attempt ≈ target」的資料上加碼。
- **replay-retry-009**（離線重播，不訓練、不叫旗艦）：拿 e2e-014 裡 14 個第一次嘗試就被退件的呼叫（01:0、09:0、14:0–3、48:0–2、55:0、78:0–1、94:0、94:2），從錄下的第一次嘗試和它的退件訊息開始，只換重試的解碼方式，各再試最多 2 次。fix points 從 log 的合併 prompt 還原（每次修訂算一點）。三種模式：G＝現行的修訂回合（看得到被退件的嘗試），greedy；T＝同樣的修訂回合，temp 0.7／top_p 0.95；F＝不給看嘗試，只用 `dsl_messages(prompt)` 重新抽樣，temp 0.7／top_p 0.95。腳本 `replay_retry.py`（scratchpad）。

  | 模式 | 重試 | 照抄 | 有改 | 清掉 | 呼叫清掉 | 有可編譯 source | fixture 全綠的呼叫 |
  |---|---|---|---|---|---|---|---|
  | G（對照） | 27 | 21 | 5 | 1 | 1/14 | 7/14 | 55:0 |
  | T（修訂＋抽樣） | 27 | 19 | 6 | 2 | 2/14 | 7/14 | 55:0、48:2 |
  | F（重新抽樣） | 22 | 0 | 14 | 8 | 8/14 | 12/14 | — |

  - G 和 e2e-014 錄到的 27／21／5／1 完全一致，重播忠實重現了迴圈，照抄不是旗艦 prompt 變異造成的。
  - T：temp 0.7 還是 19/27 一字不差地照抄，修訂回合裡「複製上一份」的機率幾乎是 1，改解碼溫度拆不掉。唯一的收穫是 48 最後一次呼叫第二次重試抽到修正版，fixture 7/7。
  - F：不照抄，清掉 8/14、可編譯 12/14；14 的四次呼叫全都跳出了 `validate_by_*` 退化（fixture 1/8、6/8、0/8、6/8）。但代價是丟掉原稿寫對的部分：各題最後一次呼叫的 fixture，48 是 G 6/7 → F 3/7，78 是 4/5 → 3/5，94 都是 1/6；沒有一題全綠。
  - 模式無關的：09 三種模式都退化成 `ConfigDict` 裡 `validate_by_*` 重複關鍵字（F 也一樣），是模型本身的先驗，不是重試格式；78 的訊息引號三種模式都沒修掉。
  - 判讀：照抄是修訂回合這個格式造成的，不是解碼。compile 類退件（沒有 source）用 F 明顯較好；conformance 類退件（source 已大致正確，只差一句訊息或一個 fix point）照抄反而保住了其他正確內容，F 會倒退。
- **som-dsl-010：`raises` 訊息只能從 prompt 抄**。009 設定不變，只加 `--swaps`（`swap_pairs`）。
  - 起因：e2e-014 的 48 在第一次嘗試就把 `binary in {0, 1}` 寫成 `y_true must contain only 0 and 1`，72（e2e-013）漏掉 `below required`。錯法都是從相似 family 回想訊息，而不是從 prompt 照抄；重試又救不回來（上表 `raises` 類照抄 10/10），所以要在第一次嘗試就修。
  - 做法：取 mechanical prompt 的 `raises` 裡帶空白的字面片段（`_worded_runs`）。只有當片段在 gold DSL 中每次出現都位於字串 token 內（`tokenize` 計數），才處理這個片段。把片段裡的內容字換成固定字表 `SWAP_WORDS` 裡的其他字，保留大小寫；功能字（`must`、`least`、`below` 等）不換。prompt 和 gold 用同一張對照表一起換：prompt 端允許字元前有反斜線，所以 regex 形式的 `\{0, 1\}` 也對得上。換完後再跑 `rejection`，只保留沒有 diagnostics、未兌現點不多於 gold 的變體。每份 prompt（mechanical 加 3 份旗艦版）各出一個變體，各用自己的亂數種子。
  - 字表寫死在程式裡，不從語料收集，所以其他 row（包括 holdout）的措辭都不會流進來。這是通用的「照抄 prompt 字面值」技能，不是針對單一 family 的規則。
  - 不處理的情形：片段是 f-string 形狀，例如 78 的 `Sensitive claim 'password'` 對應 `'{key}'`，因為片段不會逐字出現在 gold 裡。
  - 數量：非 holdout 283 個 row 產生 431 個 pair，來自 109 個 family（加入功能字保留之前量的）。實際進訓練的：train 3,988 pair，比 009 多 369；val 多 27 個 `dsl_swap`。val `dsl` 仍是同一組 224 個，可以和 009 直接比。
  - 訓練結果：val dsl loss 0.955（0）→ 0.373（50）→ 0.352（100）→ 0.345（150）→ 0.336（200）→ 0.341（250）→ **0.3278（300，最佳，已存）**。每個評估點都比 009 低（009 最佳 0.3373），但 loss 是整份 DSL 的 token 平均，raises 訊息只佔極少數 token，swap 有沒有生效要看 e2e 第一次嘗試是否照抄訊息。val `dsl_swap` 1.006 → 0.341，`dsl_fix` 0.0046、`dsl_retry` 0.0024 與 009 持平。5,064 s（009 是 6,188 s），peak 50.8 GB。
- **e2e-015**（som-dsl-010，EFFORT=low，JOBS=1）：fixture **11/18**，旗艦輸出 **8,779 token**，$1.24。
  - 翻轉：14、78 轉綠；40、43、54、55 轉紅。48、94、99 仍紅。
  - swap 有部分生效。48 第一次嘗試就把 `binary in {0, 1}` 逐字抄對，但 fixture 仍紅，原因是 `f1_score` 帶了 mock 不收的 `pos_label` kwarg，屬於邏輯錯。72 第一次呼叫的 3 次嘗試都漏掉 `below required`。旗艦下一輪加了 `fix:`，那一輪的 source 過了 fixture，但 fix point 仍被判未兌現。78 第一次嘗試兩度被退件，都是 f-string 形狀的訊息（swap 不處理的情形），第三次才寫對。
  - 4 題轉紅都和訊息無關：
    - 40：`with pipe:`，mock pipeline 不支援 context manager；`fix:` 那一輪 3 次嘗試全部照抄。
    - 43：`pd.Series[float]` 標註在執行期報 `TypeError: type 'Series' is not subscriptable`。`compile_dsl` 只會把已經存在的 `from __future__ import annotations` 提到檔頭，不會自己加（`dsl.py:385`）；gold 225 用 `pd.Series[int]` 是靠自帶 future import 才能跑，303 個 gold family 裡有 120 個帶這行。這是編譯器的通用缺口，不是模型的錯。
    - 54、55：驗證順序和 env 插值的邏輯錯。
  - 仍紅：94 `MockStreamContext` 不可迭代，`fix:` 沒被遵守；99 `KeyError: 'is_complete'`。
  - 重試統計：

    | run | adapter | 重試 | 照抄 | 有改 | 清掉 | `raises` 照抄 | compile 照抄 |
    |---|---|---|---|---|---|---|---|
    | e2e-015 | 010（swap） | 14 | 8 | 1 | 5 | 2/5 | 0/2 |

    `raises` 照抄從 10/10 降到 2/5，但照抄集中到 `fix:` 類：7 次重試裡 6 次一字不差（01、40、72 各 2 次），只有 10 清掉。`fix:` 類照抄在各輪都一樣：e2e-012（007，沒有 retry pair）2/2、e2e-014 2/2、e2e-015 6/7。所以它不是 retry pair 造成的，比較可能來自 `dsl_fix` pair 本身。那些 pair 的 previous 是 near-miss，和 target 只差一處，val loss 0.0046 表示模型幾乎只是在照抄。
  - 11 對 13 可能只是旗艦 prompt 的變異，要拿同一批 prompt 換 adapter 才分得出來，見下一項 replay。
- **編譯器修正：eager annotation 無法在執行期 subscript 時，自動補 `from __future__ import annotations`**（`dsl.py` `_unsubscriptable`、`_eager_annotations`）。
  - 規則：Python 在定義時就會 evaluate 的 annotation 包括所有參數與回傳值，以及 module 和 class 層級的 `x: T`；函式內的 `x: T` 不會。這類 annotation 裡如果 subscript 了 import 進來的物件，就在語料的直譯器裡探測：該物件沒有 `__class_getitem__`，它的 type 也沒有 `__getitem__` 時，才補上 future import。
  - 探測結果：`pd.Series[float]` 會補；`asyncio.Queue[int]`、`subprocess.Popen[str]`、`st.SearchStrategy[...]` 都可以 subscript，不補。gold 裡沒帶 future import 的 4 個 family 正好都是後者，所以 round-trip gate 不受影響，161 個測試全綠。`to_dsl` 不動，訓練 target 也就不變。
  - 驗證：把 e2e-015 的 43 用原本的 DSL 和 exports 重新編譯，fixture 8/8，quality 乾淨。這是通用的「編譯產物要能 import」修正，不是針對 43 的規則，但 43 是 holdout，所以之後的 e2e 數字要註明是加上這個修正之後量的。
- **成對重播（paired replay）**（`replay_prompts.py`，scratchpad）：旗艦 prompt 固定，把一次 e2e 錄下的 `som_generate` 呼叫依序送進另一個 adapter，重現 `som_generate` 的行為：`fix:` 修訂帶累積的 fix points 和這次重播自己的上一份 DSL。每次呼叫都用 fixture 判。greedy 解碼，拿 run 自己的 adapter 重播會逐位元組重現原 run，已驗證。
  - P＝prompt 組（P14＝e2e-014 錄的，P15＝e2e-015 錄的），A＝adapter（A9＝009，A10＝010）。兩組重播都已經帶上面的編譯器修正；e2e-015 本身沒有，43 補修正後算綠。

    | prompt | A9 | A10 | 只有 A9 綠 | 只有 A10 綠 |
    |---|---|---|---|---|
    | P14 | 13（e2e-014） | 9 | 29、40、55、72、73 | 78 |
    | P15 | 11 | 12（e2e-015＋43） | 40、54、94 | 01、31、72、78 |
    | 合計 | 24/36 | 21/36 | 8 | 5 |

  - 每組 prompt 都偏袒自己的 adapter，因為旗艦的修訂是照那個 adapter 的輸出寫的。即使如此，A10 在自己的 P15 上也只多 1 題。swap 沒有帶來可見的提升，淨值偏負；8 對 5 的 sign test p≈0.58，不顯著。
  - 雜訊比訊號大：只改一個資料變數，36 組裡就有 13 組（36%）翻轉。18 題的單次 e2e 分不出 ±3 題以內的差距。之後比較 adapter 先跑成對重播（不花旗艦費用，兩組 prompt 各跑一次），e2e 只用來量旗艦 token。
  - A10 在 P14 輸的 5 題，有 4 題第一次嘗試就編譯乾淨，但語意錯。例如 73 的 intent 寫明用 `AESGCM`，兩個 adapter 都改寫成 `Cipher(..., modes.GCM)`（從相近 family 回想）；A10 還把一個 encryptor 共用到每次呼叫，並把訊息寫成 `f"Key must be {sorted(VALID_KEY_LENGTHS)} bytes"`。f-string 形狀的訊息 `rejection` 不檢查，所以沒被退件。
- **修訂第一回合不照抄，照抄只發生在同 prompt 的重試**：`fix:` 修訂的第一回合 prompt 會變，e2e-012 到 e2e-015 每輪只有 1–2 次和前一份一字不差（1/25、1/20、2/23、1/27）。同一個 prompt 的迴圈內重試才會照抄：e2e-015 `fix:` 類 6/7，e2e-014 `raises` 類 10/10。照抄跟著「prompt 有沒有變」走，不是跟著 feedback 的內容走。
- **M 模式重播：把退件訊息併進 intent，照抄不變**（`replay_retry.py` 模式 M）：上一項推測照抄跟著「prompt 有沒有變」走，所以重試時把退件訊息接到 prompt 的 intent 後面，讓 prompt 變得像 `fix:` 修訂的第一回合；其餘和 G 相同，greedy。

    | 重播 | adapter | 呼叫 | 重試 | 照抄 | 有改 | 清掉 | 清掉的呼叫 |
    |---|---|---|---|---|---|---|---|
    | e2e-015 G | 010 | 9 | 14 | 8 | 1 | 5 | 10:1、62:0、62:1、73:1、78:0 |
    | e2e-015 M | 010 | 9 | 14 | 9 | 1 | 4 | 10:1、62:0、62:1、73:1 |
    | e2e-014 G | 009 | 14 | 27 | 21 | 5 | 1 | 55:0 |
    | e2e-014 M | 009 | 14 | 26 | 20 | 3 | 3 | 01:0、09:0、55:0 |

  - 合計 G 照抄 29/41、清掉 6 次呼叫；M 照抄 29/40、清掉 7 次呼叫，沒有差別。假說不成立：修訂第一回合之所以會改，不是因為 prompt 變了。比較可能的解釋是，第一回合的 previous 是上一次呼叫已經通過的 DSL，`fix:` 要求的是模型做得到的改動；迴圈內重試的 previous 則是模型剛寫出、而且已經寫不對的東西，模型不知道怎麼改時就原樣吐回。推論端改 prompt 的形式救不了這種情況，`generate_module` 維持不動。
- **發現：007 到 010 都只看過 7.5% 的訓練資料**。四輪都是 `--iters 300`、batch 1，每一步只取 1 個 pair，300 步只看得到 3,988 個 train pair 裡的 300 個。`iterate_batches` 用固定 seed 洗牌，但 pair 集合一變，洗出來的順序就完全不同，所以 009 和 010 看到的其實是兩組不重疊的隨機 300 個樣本。swap（369 個，9%）大約被抽到 28 次；`dsl_retry`（約 304 個，從 007→009 的 pair 數差推算）大約 23 次。上面的「只改一個資料變數」其實同時換掉了 300 個樣本，成對重播 36% 的翻轉大部分應該來自這裡。
  - 時間都花在 eval：`memory.jsonl` 顯示訓練每步約 2.5 s，300 步共約 750 s。每次 eval 要跑 424 個 val pair，約 620 s，每 50 步一次，一共 7 次，約 4,300 s，占 010 總時間 5,064 s 的 85%。
  - `train()` 改成每次 best 改善就寫出 `adapters.safetensors`，`adapter_config.json` 在第 1 步前先寫好。這樣跑好幾小時的 run 就算遇到機器重開，也留得下可以載入的 checkpoint（9/26 已經發生過一次）。
- **som-dsl-011：010 設定不變，`--iters 300` 改成 4000（約一個 epoch）**，`--eval-every` 從 50 改成 500，只影響量測時間。前 300 步和 010 用同一個 seed、同一組 pair，順序完全相同。預估訓練 2.8 h，加上 9 次 eval 1.6 h。訓練完先拿 P14、P15 兩組 prompt 跑成對重播，和 A9、A10 的 24/36、21/36 比。
- **011 的結果：val 最低點落在 300～500 步，之後一路上升，所以在 step 1500 停掉。** step 0 的 val 和 010 逐位相同。這證實兩次 run 的前 300 步完全一樣，010 就是 011 的前段。

  | step | train | dsl | dsl_fix | dsl_retry | dsl_swap | 四層平均 |
  |---|---|---|---|---|---|---|
  | 300（010） | 0.2024 | **0.3278** | 0.0046 | 0.0024 | **0.3411** | **0.1690** |
  | 500 | 0.1710 | 0.3455 | 0.0043 | 0.0020 | 0.3630 | 0.1787 |
  | 1000 | 0.0636 | 0.3651 | 0.0045 | 0.0019 | 0.3756 | 0.1868 |
  | 1500 | 0.0284 | 0.4247 | 0.0049 | 0.0022 | 0.4424 | 0.2186 |

  - train 從 0.17 降到 0.03，`dsl`、`dsl_swap` 的 val 卻從 0.35 升到 0.43。這是背下訓練 family，不是學會可以推廣的東西。
  - `dsl_fix`、`dsl_retry` 幾乎不動。這兩層的 target 大多已經寫在輸入裡（上一版 DSL 加一個修正點），第 50 步就學會了。
  - 所以「只看過 7.5% 資料」不是瓶頸。不同 family 的程式沒辦法從 prompt 推出來的部分，多看同一批 255 個 family 也補不起來。這和 fixture 失敗的主因一致：模型回想的是相鄰 family 的寫法。
  - 下一個能動的變數是 family 數量，也就是造更多不同的題目，而不是步數。
  - 停損規則是事先定好的：step 1500 的四層平均沒低於 step 500 就停。存下來的 checkpoint 是 step 500，它是 011 這次 run 的最佳點，但 val 還是比 010 的 step 300 差。拿它跑 P14、P15 成對重播，確認 val 的差距會不會反映到 fixture 上。
- **A11（011 的 step 500）成對重播：P14 8/18，P15 11/18，合計 19/36。** 對照 A9 24/36、A10 21/36（編譯器修正後）。
  - A10 和 A11 在 P14 各有 4／3 題只有自己過，在 P15 各 2 題，互有勝負。
  - val 高一點的 A11（0.3455，A10 是 0.3278），fixture 也少兩題，方向一致，但落在 36% 翻轉率的雜訊範圍內。
  - 三個 adapter 在同一組 prompt 上都是 19～24／36，只換 adapter 的成對重播已經分不出它們。要讓 fixture 明顯改變，得靠 val 大幅下降的變數，也就是 012／013 要確認的 family 數量。
- **som-dsl-012／013：只改 family 數量，看 val 最低點會不會跟著 family 變多而下降。** 012 用 25%（64 個）訓練 family，013 用 50%（128 個）。小比例的 family 集合一定包含在大比例裡（`subset_families`：同一個 seed 洗牌後取前段）。val 仍然是同樣 28 個 family，所以三條曲線量的是同一件事。100% 那條用 010 加 011 的曲線。其餘設定和 010 相同；`--iters 600`、`--eval-every 50`，並用 `--val-layer dsl` 只量 `dsl` 層，eval 時間約減半。判讀方式：
  - dsl val 最低值從 25% 到 50% 再到 100% 明顯下降：瓶頸在 family 數量，下一步是造題，並用這條曲線外推要造多少題。
  - 最低值幾乎不動：瓶頸在 prompt 給的資訊不夠，多造題也沒用，要改的是 prompt 語言或 DSL。
- **012／013 的結果：family 每翻一倍，dsl val 最低值降約 0.018，和 log(family 數) 大致成直線。** 兩次 run 都在確認過擬合後手動停掉：012 在 step 200、013 在 step 350，val 都連續兩個點高於最低點。最低點的 checkpoint 已經存下。

  | 訓練 family | 最低 dsl val | 出現在 | 之後 |
  |---|---|---|---|
  | 25%（64） | 0.3645 | step 100 | 150：0.3940，200：0.3875 |
  | 50%（128） | 0.3456 | step 250 | 300：0.3765，350：0.3579 |
  | 100%（255，010） | 0.3278 | step 300 | 500：0.3455 |

  - 25%→50% 降 0.0189，50%→100% 降 0.0178。family 越多，過擬合也來得越晚（100→250→300 步）。
  - 方向是「family 有用」，但斜率很平。翻一倍換到的 0.018，跟 A10（0.3278）和 A11（0.3455）的 val 差距一樣大，而這兩者在成對重播中分不出來（21 vs 19／36）。照這個斜率外推，510 個 family 約 0.31，1,020 個約 0.29，要到 0.25 得有約 5,000 個 family。
  - 沒有量過不同 seed 之間的變異，0.018 的差距有多少是雜訊並不清楚。不過三個點單調遞減，而且過擬合點也一起往後移。
  - 所以 family 數量不是主要瓶頸：以做得到的規模多造 family，val 只會慢慢降。0.33 裡面大部分是什麼要先拆清楚，下一步是在 val 上逐 token 歸因 loss。
- **逐 token 歸因：dsl val loss 有一半以上花在 docstring，val 在 300 步後「過擬合」的幾乎全是 docstring 和自取名稱。** 方法（scratchpad `token_loss.py`）：重建與訓練相同的 28 個 val family 和 224 個 dsl pair，用 adapter 為每個 completion token 算 loss，再依 token 起點落在哪個 Python token 分類。重算的平均和訓練 log 相差不到 0.001（010：0.3287 vs 0.3278；012：0.3655 vs 0.3645；013：0.3465 vs 0.3456）。
  - 010 各類在總 loss 中的占比：docstring 52.5%（token 數占 14%，平均 1.22）、運算子與標點 12.9%、自取名稱（def／參數／賦值目標，prompt 裡沒有的）12.4%、其他字串 6.1%、prompt 裡出現過的名稱 6.5%，keyword／函式庫名稱／屬性合計 7%。
  - 下表中，「語意」是 fixture 看得到的部分，也就是扣掉 docstring、自取名稱、註解後剩下的 token；「自由」是被扣掉的那三類。

    | adapter | 全部 | 語意 | 自由 |
    |---|---|---|---|
    | 25% @100（012） | 0.3655 | 0.1636 | 1.146 |
    | 50% @250（013） | 0.3465 | 0.1543 | 1.090 |
    | 100% @300（010） | 0.3287 | **0.1434** | 1.045 |
    | 100% @500（011） | 0.3464 | 0.1456 | 1.123 |

  - 010 到 011（step 300→500）：全部 +0.018，語意只 +0.002，自由 +0.078。011 的「過擬合」幾乎都是在背訓練 family 的 docstring 和名稱，語意部分從 step 300 起就持平。多訓練不會讓語意變好，也不會變差。
  - family 曲線換成只算語意：每翻一倍降 0.009～0.011（相對 6～7%），仍然和 log 成直線，斜率和全部一起算差不多。
  - 所以 val loss 的絕對值大半是 fixture 不在乎的文字，拿它挑 checkpoint 或比較 adapter 時會被 docstring 帶著走。語意部分的平均是 0.143，其中運算子與標點占 37%、prompt 裡有的名稱 19%、其他字串 18%、keyword 11%。
- **語意 loss 大多在函式本體，簽章只占 15%。** 同一次重算（010）再依字元所在的最內層 statement 分區，並把 def／class 的標頭（decorator、簽章、型別標註）和本體分開：賦值 34.5%（平均 0.227）、簽章 15.1%（0.111）、raise 11.8%（0.184）、控制流程標頭 10.7%（0.146）、運算式 9.2%（0.231）、類別欄位 7.5%、return 7.0%、decorator 與 class 標頭各 1.7%。
  - 把簽章寫進 prompt，最多只能省下語意 loss 的 15%，而旗艦輸出 token 會跟著增加，不做。
  - 剩下的是本體的實作邏輯，而這正是 SOM 模型該自己產生的部分。目前唯一量到有效的變數是 family 數量，每翻一倍約 7%。
  - 接下來要把 loss 換算成 fixture。A10 和 A11 的語意 loss 幾乎一樣（0.1434 vs 0.1456），成對重播也分不出來（21 vs 19／36），兩者一致。所以拿 012（25%，語意 0.1636）和 013（50%，0.1543）在 P14、P15 跑成對重播：如果 fixture 跟著語意 loss 往下掉，就能算出每翻一倍 family 能多過幾題。
- **A12（012，25%）／A13（013，50%）成對重播：fixture 不隨 family 數量變，A12 21/36、A13 20/36、A10 21/36。** 五個 adapter 放在一起（09、10 沒有 fixture 不計，14 沒產出算紅；星號是錄下該組 prompt 的 adapter）：

  | prompt | A9 | A10（100%） | A11（100%@500） | A12（25%） | A13（50%） |
  |---|---|---|---|---|---|
  | P14 | 13* | 9 | 8 | 10 | 9 |
  | P15 | 11 | 12* | 11 | 11 | 11 |
  | 合計 | 24 | 21 | 19 | 21 | 20 |
  | 語意 val loss | — | 0.1434 | 0.1456 | 0.1636 | 0.1543 |

  - 對 A10 的成對比較：A12 各有 6 題只有自己過（p=1.00），A13 是 5 對 4（p=1.00），A11 是 6 對 4（p=0.75），A9 是 5 對 8（p=0.58）。語意 loss 差了 14%（A12 對 A10），fixture 一題都沒差。
  - 36 組裡 11 組五個 adapter 都過，6 組都不過（P14 的 14、48、99，P15 的 48、55、99），19 組（53%）在 adapter 之間翻轉。P15 更極端，除了 A10 自己多 1 題，四個 adapter 都是 11，但過的是不同題。
  - 所以在 64～255 個 family、語意 loss 0.14～0.16 這個範圍，fixture 對 adapter 不敏感，分數由 prompt 組和逐題的隨機翻轉決定。每組 18 題的成對重播看不出 ±2 題以內的差異，拿它校準「loss 降多少換幾題」做不到。
  - 下一步是離線 val-fixture 評測（scratchpad `val_fixture_eval.py`）：28 個 val family，每個用機械 prompt 加 3 份旗艦 recap prompt，都走 `generate_module`，再用 fixture 判。每個 adapter 有 112 組配對結果，不花旗艦費用，也不碰 holdout。先跑 A10、A12、A13、A11。
- **離線 val-fixture：fixture 隨 family 數量單調上升，成對重播看不出來的差距在這裡顯著。** 每個 prompt 只送一次 `generate_module`（greedy，含它自己的重試），不經旗艦修訂。除了全綠題數，也記 fixture 測試通過的比例（從 pytest 摘要解析，全綠算 1），這是比二元結果更靈敏的連續指標（scratchpad `val_matrix.py`）。

  | adapter | 機械 | r0 | r1 | r2 | 全綠 | 測試通過比例 | 編譯乾淨 |
  |---|---|---|---|---|---|---|---|
  | A12（25%，64 family，@100） | 7 | 10 | 6 | 1 | 24/112（21%） | 0.470 | 91 |
  | A14（25%，64 family，@300） | 8 | 10 | 5 | 6 | 29/112（26%） | 0.467 | 83 |
  | A13（50%，128 family，@250） | 8 | 9 | 6 | 6 | 29/112（26%） | 0.497 | 99 |
  | A10（100%，255 family，@300） | 6 | 11 | 10 | 8 | **35/112（31%）** | **0.552** | 103 |

  - 對 A10 的成對比較：A12 只有 A10 綠 17、只有 A12 綠 6（p=0.035），測試通過比例較低 39、較高 19（p=0.012）。A13 是 9 對 3（p=0.15），比例 30 對 15（p=0.036）。
  - 同樣兩個 adapter 在 holdout 成對重播是 21 對 21。每組 18 題的重播被雜訊蓋過，112 組配對才量得出來。之後比較 adapter 都用這個評測，holdout e2e 只用來量最終分數和旗艦 token。
  - family 每翻一倍，全綠約多 5 題（+4.5 個百分點），測試通過比例多 0.03～0.055。family 越少，對 prompt 寫法越脆弱：A12 在 r2 只過 1/28，其中 5 題三次嘗試都沒編譯過；A10 在 r2 過 8/28。
  - 照 log 直線外推，510 個 family 約 36%，1,020 個約 41%，4,000 個約 50%。單靠造題追不上 holdout 18/18。（這條斜率沒控制步數，同步數下只剩約一半，見下方 A14。）
  - 11 個 family 三個 adapter、四種 prompt 全紅，大多是一次就編譯乾淨、語意卻錯。例如 292：prompt 寫明例外訊息 `'APP_DATABASE_URL is required'`、範圍檢查和 `from None`，模型寫出通用的 `f"{name}: {message}"`，沒有範圍檢查。f-string 形式的訊息 `rejection` 不檢查，所以沒被退件。資訊都在 prompt 裡，是模型沒照著做。
  - 機械 prompt（6～8/28）不比旗艦 recap（r0 9～11）好。欄位齊全不代表比較好翻。
  - val 單次 31% 遠低於 holdout e2e 的約 60%，兩個因素還沒拆開：e2e 有旗艦的 `fix:` 修訂，這裡只有單次生成；val 裡 100 號以後的 family 契約比較長。
  - A11（100%，@500）：27/112，測試通過比例 0.520，機械 7、r0 8、r1 6、r2 6。對 A10：只有 A10 綠 9、只有 A11 綠 1（p=0.021），比例較低 25、較高 11（p=0.029）。同一次 run 多訓練 200 步，語意 val loss 只差 0.002（0.1434 對 0.1456），fixture 卻少 8 題。語意 loss 平均看不出這個退化，300 步之後的「背 docstring」會連帶傷到照 prompt 寫的能力。挑 checkpoint 要看 val-fixture，不看 val loss。
  - 混淆變數：三個 checkpoint 都是依整體 val loss 選的，步數分別是 100、250、300，而整體 val loss 主要由 docstring 主導。所以上表同時改了 family 數量和訓練步數。som-dsl-014 用 25% family 訓練到 step 300（固定學習率、同一個 seed，前 100 步和 012 完全相同），拿來和 A10 比，拆開 family 數量和步數的效果。
  - A14（som-dsl-014，25% family 訓練到 step 300）拆開了這兩個效果。step 300 的整體 val loss 是 0.4346，遠高於 012 在 step 100 的最低點 0.3645；只看 loss 早就過擬合了，fixture 卻沒有變差。
    - 加步數（A12→A14，同樣 64 個 family）：全綠 24→29，只有 A14 綠 14、只有 A12 綠 9（p=0.41）。測試通過比例 0.470→0.467，較高 28、較低 33（p=0.61）。步數對通過比例沒有影響。
    - 加 family（A14→A10，同樣 300 步）：全綠 29→35，只有 A10 綠 16、只有 A14 綠 10（p=0.33）。測試通過比例 0.467→0.552，較低 43、較高 20（p=0.005）。編譯乾淨 83→103。
    - 結論：同步數下 family 數量仍然有效，顯著的是測試通過比例和編譯乾淨數，全綠差距從 11 題縮到 6 題，不顯著。上面「翻一倍多 5 題」約一半來自步數，同步數下約 3 題（+2.7 個百分點）。照這個斜率，4,000 個 family 也只到約 42%，更追不上 holdout。
    - family 少又訓練久，結果會變得很看題：A14 在 139 過 3/4、236 過 4/4（其他 adapter 大多 0～2），但在 101、57、66、204 只有 0～1/4（A10 是 2～4）。編譯乾淨只有 83，是五個 adapter 裡最低的。
- **A15（som-dsl-015，DSL target 不含 docstring）：沒有變好，方向反而略差。** 只改一個變數：`--no-docstrings` 把函式和 class 的 docstring 從每個 completion，以及 retry turn 裡被退件的 attempt 拿掉（`som_core.dsl.drop_docstrings`，訓練與 val 兩側都拿），其餘設定和 010 相同（100% family、300 步、3,988 個 pair）。303 個 family 拿掉後都能無診斷編譯，DSL 短了 16.7%。step 300 的 val dsl loss 是 0.1989，但它不含 docstring，不能和 010 的 0.3278 比。
  - val-fixture：機械 9、r0 8、r1 7、r2 6，共 30/112（A10 35）。只有 A10 綠 10、只有 A15 綠 5（p=0.30）；測試通過比例 0.487 對 0.552，較高 18、較低 27（p=0.23）。編譯乾淨 91 對 103。綠的 30 題全都有 pylint 發現（缺 docstring），A10 有 2 題 pylint 全過。
  - 機械 prompt 從 6 升到 9，三份 recap 都下降。差距不顯著，但沒有任何一項指標支持「docstring 佔掉一半 loss，所以拖累語意」。它佔的是 loss，不是能力。docstring 也可能是函式本體之前的一小段計畫，拿掉就少了這一步。之後的 target 保留 docstring。
- **A16（som-dsl-016，不做條件 dropout）：顯著變差。** 只改 `--no-dropout`，其餘和 010 相同。訓練 pair 從 3,988 降到 2,968，step 300 的 val dsl loss 0.3453（010 是 0.3278）。
  - val-fixture：機械 9、r0 7、r1 6、r2 6，共 28/112（A10 35）。只有 A10 綠 15、只有 A16 綠 8（p=0.21）；測試通過比例 0.468 對 0.552，較高 17、較低 38（p=0.006）。編譯乾淨 100 對 103。
  - 有干擾因子：pair 變少，同樣 300 步裡每個 family 的 dsl pair 曝光約 103 次，010 約 153 次。所以不能分清是 dropout 本身有用，還是只是曝光比較多。無論哪一種，dropout 都保留。
  - A14、A15、A16 都落在 28～30，只有 A10 是 35。A10 可能是偏高的一次，同設定換 batch 順序的變異還沒量過，單變數比較的差距要先扣掉這個變異。
- **A17（som-dsl-017，A10 設定只換 batch 順序，`--order-seed 43`）：A10 不是運氣。** pair 和 010 完全相同（3,988），step 300 的 val dsl loss 0.3387（010 是 0.3278）。
  - val-fixture：機械 9、r0 11、r1 10、r2 6，共 36/112（A10 35）。只有 A10 綠 6、只有 A17 綠 7（p=1.00）；測試通過比例 0.526 對 0.552，較高 18、較低 23（p=0.53）。編譯乾淨 104 對 103。
  - 這就是同設定重跑的雜訊：全綠差 ±1，不一致的綠約 13 題，通過比例差約 0.03。A14、A16 的通過比例是較低 38～43 對較高 17～20，遠大於這個雜訊，是真的變差。A15（較低 27、較高 18）和 A11（27/112）則落在雜訊邊緣。
  - batch 1、300 步只看 300 個 pair，約是 3,988 的 7.5%，每個 family 平均約一個。所以 300 步內「看到哪些種類的 pair」就是訓練內容：拿掉 dropout 會讓 dsl pair 從約 153 降到約 103；加步數（A11）是重看同一批 family，反而變差。
