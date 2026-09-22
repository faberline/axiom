# SOM (Structured Outcome Model) 架構設計：SQL引擎哲學與多層次解壓

## Goal Description
本架構精準借鏡 SQL Database 引擎的運作哲學，透過多層次解壓 (Density Decompression) 將自然語言平滑展開為實體組裝代碼。同時，我們確立了嚴格的「輸入邊界限制」，確保 SOM 專注於高內聚的執行任務，防範過大範圍的請求導致管線崩潰。

## User Review Required
> [!IMPORTANT]
> 已根據您的提問，正式定義了 **「推薦任務粒度 (Recommended Task Scope)」**。我們將界限定義在「單一垂直功能切片 (Vertical Feature Slice)」，這能完美發揮 Layer 2 跨檔案拓撲的威力，同時避免 Layer 3 組裝樹過度膨脹。

## 任務邊界與輸入限制 (Scope Limits & Boundary Control)

### 推薦的任務粒度 (The Sweet Spot)
我們強烈建議前端 Agent 傳入的任務大小應該是 **「單一垂直功能切片 (Vertical Feature Slice)」**。
這比單純的「一個 function」大，但比「一個子系統」小。

* **理想範圍範例**：`實作使用者註冊功能 (User Registration API)`
  這通常會涵蓋：
  1. 1~2 個 Data Model (例如 `UserCreate` Pydantic Schema)
  2. 1 個 FastAPI Router (`POST /register`)
  3. 1 個資料庫/業務邏輯操作 (密碼雜湊與寫入 DB)
* **為什麼可以這麼大？**
  因為我們特別設計了 **Layer 2 (Component Topology)**！如果每次只做一個 Function，那根本不需要這層「跨檔案的架構分配表」。Layer 2 的存在就是為了能一口氣協調 `models.py`, `routes.py`, `crud.py` 之間的積木依賴關係。
* **為什麼不能再更大？**
  如果任務變成 `實作整套會員系統 (包含 OAuth, Email 驗證, 權限管理)`，那麼 Layer 3 (實體組裝層) 所需產出的「積木參數樹」會暴增到數百個節點，這會嚴重超出目前微調模型的可靠注意力區間 (Context Window)，導致拼裝錯亂。

### 守門員機制 (Gatekeeper)
Planner 會動態評估這項任務。若判斷單次請求會牽涉超過 5 個以上的實體檔案變更，或是超過 20 塊核心積木，就會觸發 `SCOPE_TOO_LARGE` 拒絕處理，強制上游 Agent 拆分成多個「垂直功能切片」。

---

## Proposed Pipeline Stages (4-Stage Decompression)

### Layer 1: Planner (守門員與極簡意圖翻譯機)
- **職責**: 負責防守輸入邊界，將合理大小的自然語言翻譯為**極簡中介語言 (Minimal Custom DSL)**。
- **輸出範例 (抽象意圖)**:
  ```yaml
  Intent: "Create Registration API"
  Target: "User"
  Constraints: ["Hash password", "Return 400 on duplicate email"]
  ```

### Layer 2: SOM Component Topology (架構與模組映射緩衝層)
- **職責**: 將 Layer 1 的商業意圖，解壓為專案的實體物理拓撲圖。跨檔案分配任務。
- **輸出範例 (拓撲展開)**:
  ```yaml
  Files:
    - path: "app/models/user.py"
      blocks: ["UserCreate Schema"]
    - path: "app/routes/auth.py"
      blocks: ["POST /register Route"]
    - path: "app/services/auth_svc.py"
      blocks: ["Password Hash", "DB Insert"]
  ```

### Layer 3: SOM Query Optimizer (實體組裝序列展開)
- **職責**: 接收 Layer 2 的拓撲圖，精確輸出積木 ID 與參數 (Physical Assembly Sequence)。
- **輸出範例 (實體指令)**:
  ```json
  [
    {"op": "CREATE_FILE", "path": "app/routes/auth.py"},
    {"op": "INSERT_SNIPPET", "id": "fastapi_router_base"},
    {"op": "INSERT_SNIPPET", "id": "route_post_json", "params": {"path": "/register"}}
  ]
  ```

### Layer 4: Execution Engine (Rust Assembler)
- **職責**: 接收 Layer 3 的序列，將積木高速無腦拼接，輸出完美的 Python 程式碼。量產期將以極速的 Rust 實作。
