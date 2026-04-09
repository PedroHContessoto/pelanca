import json

nb = json.load(open('D:/pelanca/training/pelanca_nnue_v2_kaggle.ipynb'))

# Cell 0: Title
nb['cells'][0]['source'] = ["# Pelanca NNUE v3\n","\n","- Hidden 16, Batch 64K, MSE puro, sem material anchor"]

# Cell 2: Config
nb['cells'][2]['source'] = [
"MIN_DEPTH = 18\n","MAX_ABS_CP = 3000\n","EVAL_SCALE = 400.0\n",
"HDF5_FILE = '/kaggle/working/positions_v2.h5'\n",
"RAW_DIR = '/kaggle/working/parquets'\n","os.makedirs(RAW_DIR, exist_ok=True)\n","\n",
"PARQUET_IDS = ['00015', '00016']\n",
"BASE_URL = 'https://huggingface.co/datasets/Lichess/chess-position-evaluations/resolve/main/data'\n","\n",
"INPUT_SIZE = 768\n","FT_SIZE = 256\n","HIDDEN_SIZE = 16\n","\n",
"EPOCHS = 80\n","BATCH_SIZE = 65536\n","LR_MAX = 1e-3\n","LR_MIN = 1e-5\n",
"WEIGHT_DECAY = 1e-6\n","MIRROR_AUG = True\n","\n",
"FT_QUANT = 64\n","HIDDEN_QUANT = 64\n","OUTPUT_QUANT = 64\n","\n",
"NNUE_OUTPUT = '/kaggle/working/pelanca_v3.nnue'\n",
"CKPT_DIR = '/kaggle/working/checkpoints_v3'\n","os.makedirs(CKPT_DIR, exist_ok=True)\n","\n",
"params = INPUT_SIZE*FT_SIZE + FT_SIZE + FT_SIZE*2*HIDDEN_SIZE + HIDDEN_SIZE + HIDDEN_SIZE + 1\n",
"print(f'Arch: {INPUT_SIZE}->{FT_SIZE}->{HIDDEN_SIZE}->1 ({params:,} params)')\n",
"print(f'Batch: {BATCH_SIZE:,} | Epochs: {EPOCHS}')"
]

# Cell 6: Dataset + make_batches (NO WDL)
nb['cells'][6]['source'] = [
"_PIECE_MAP = {\n","    'P': (0, 0), 'N': (0, 1), 'B': (0, 2), 'R': (0, 3), 'Q': (0, 4), 'K': (0, 5),\n",
"    'p': (1, 0), 'n': (1, 1), 'b': (1, 2), 'r': (1, 3), 'q': (1, 4), 'k': (1, 5),\n","}\n","\n",
"def fen_to_sparse_fast(fen_str):\n",
"    if isinstance(fen_str, bytes): fen_str = fen_str.decode()\n",
"    parts = fen_str.split(' ')\n","    stm = 0 if parts[1] == 'w' else 1\n",
"    wi, bi = [], []\n","    sq = 56\n","    for ch in parts[0]:\n",
"        if ch == '/': sq -= 16\n","        elif ch.isdigit(): sq += int(ch)\n",
"        else:\n","            color, pt = _PIECE_MAP[ch]\n",
"            wi.append(color * 384 + pt * 64 + sq)\n",
"            bi.append((1 - color) * 384 + pt * 64 + (sq ^ 56))\n",
"            sq += 1\n","    return stm, wi, bi\n","\n","\n",
"class NnueSparseDatasetV3(Dataset):\n",
"    def __init__(self, h5_path, split='train', max_pos=None, mirror=False):\n",
"        import gc\n","        self.mirror = mirror\n",
"        with h5py.File(h5_path, 'r') as f:\n","            g = f[split]\n",
"            total = len(g['evals'])\n",
"            n = min(total, max_pos) if max_pos else total\n",
"            self.evals = g['evals'][:n].astype(np.float32)\n",
"        self.n = n\n",
"        print(f'[{split}] {self.n:,}' + (f' (x2={self.n*2:,})' if mirror else ''))\n",
"        max_pieces = self.n * 32\n",
"        w_flat = np.empty(max_pieces, dtype=np.int16)\n",
"        b_flat = np.empty(max_pieces, dtype=np.int16)\n",
"        offsets = np.empty(self.n + 1, dtype=np.int32)\n",
"        self.stm = np.empty(self.n, dtype=np.uint8)\n",
"        CHUNK = 500_000\n","        pos = 0\n","        offsets[0] = 0\n",
"        for cs in tqdm(range(0, self.n, CHUNK), desc=f'  {split}', total=(self.n+CHUNK-1)//CHUNK):\n",
"            ce = min(cs + CHUNK, self.n)\n",
"            with h5py.File(h5_path, 'r') as f:\n",
"                fc = f[split]['fens'][cs:ce]\n",
"            for j, rf in enumerate(fc):\n","                i = cs + j\n",
"                s, wi, bi = fen_to_sparse_fast(rf)\n","                k = len(wi)\n",
"                w_flat[pos:pos+k] = wi\n","                b_flat[pos:pos+k] = bi\n",
"                pos += k\n","                offsets[i+1] = pos\n","                self.stm[i] = s\n",
"            del fc\n","            gc.collect()\n",
"        self.w_flat = w_flat[:pos].copy()\n","        self.b_flat = b_flat[:pos].copy()\n",
"        self.offsets = offsets\n","        del w_flat, b_flat\n","        gc.collect()\n",
"        mb = (self.w_flat.nbytes+self.b_flat.nbytes+self.offsets.nbytes+self.evals.nbytes+self.stm.nbytes)/1e6\n",
"        print(f'  {mb:.0f} MB RAM')\n","\n",
"    def __len__(self): return self.n * 2 if self.mirror else self.n\n","\n","\n",
"def make_batches(ds, batch_size, shuffle=True):\n",
"    n = len(ds)\n","    perm = np.random.permutation(n) if shuffle else np.arange(n)\n",
"    for bs in range(0, n - batch_size + 1, batch_size):\n",
"        idx = perm[bs:bs+batch_size]\n",
"        mm = idx >= ds.n\n","        ri = np.where(mm, idx - ds.n, idx)\n",
"        B = len(idx)\n",
"        starts = ds.offsets[ri].astype(np.int64)\n",
"        ends = ds.offsets[ri+1].astype(np.int64)\n",
"        lengths = ends - starts\n",
"        cl = np.cumsum(lengths)\n",
"        bo = np.empty(B, dtype=np.int64)\n","        bo[0] = 0\n","        bo[1:] = cl[:-1]\n",
"        total = int(cl[-1])\n",
"        base = np.repeat(starts, lengths)\n",
"        within = np.arange(total, dtype=np.int64) - np.repeat(bo, lengths)\n",
"        flat = base + within\n",
"        aw = ds.w_flat[flat].astype(np.int64)\n",
"        ab = ds.b_flat[flat].astype(np.int64)\n",
"        if ds.mirror and mm.any():\n",
"            me = np.repeat(mm, lengths)\n","            aw[me] ^= 7\n","            ab[me] ^= 7\n",
"        blk = ds.stm[ri] == 1\n",
"        evs = ds.evals[ri].copy()\n","        evs[blk] *= -1\n",
"        if blk.any():\n",
"            be = np.repeat(blk, lengths)\n",
"            tmp = aw[be].copy()\n","            aw[be] = ab[be]\n","            ab[be] = tmp\n",
"        yield (torch.from_numpy(aw), torch.from_numpy(bo),\n",
"               torch.from_numpy(ab), torch.from_numpy(bo.copy()),\n",
"               torch.from_numpy(evs))\n","\n","print('OK')"
]

# Cell 8: Model V3
nb['cells'][8]['source'] = [
"class ClippedReLU(nn.Module):\n","    def forward(self, x): return torch.clamp(x, 0.0, 1.0)\n","\n",
"class PelancaNNUEv3(nn.Module):\n","    def __init__(self):\n","        super().__init__()\n",
"        self.ft = nn.EmbeddingBag(INPUT_SIZE, FT_SIZE, mode='sum', sparse=False)\n",
"        self.ft_bias = nn.Parameter(torch.zeros(FT_SIZE))\n",
"        self.hidden = nn.Linear(FT_SIZE * 2, HIDDEN_SIZE)\n",
"        self.out = nn.Linear(HIDDEN_SIZE, 1)\n","        self.crelu = ClippedReLU()\n",
"        self._init()\n","\n","    def _init(self):\n",
"        nn.init.kaiming_normal_(self.ft.weight, nonlinearity='relu')\n",
"        nn.init.zeros_(self.ft_bias)\n",
"        nn.init.kaiming_normal_(self.hidden.weight, nonlinearity='relu')\n",
"        nn.init.zeros_(self.hidden.bias)\n",
"        nn.init.xavier_normal_(self.out.weight)\n",
"        nn.init.zeros_(self.out.bias)\n","\n",
"    def forward(self, stm_idx, stm_off, nstm_idx, nstm_off):\n",
"        with torch.amp.autocast('cuda', enabled=False):\n",
"            stm_acc = self.crelu(self.ft(stm_idx, stm_off) + self.ft_bias)\n",
"            nstm_acc = self.crelu(self.ft(nstm_idx, nstm_off) + self.ft_bias)\n",
"        h = self.crelu(self.hidden(torch.cat([stm_acc, nstm_acc], dim=1)))\n",
"        return self.out(h)\n","\n",
"def nnue_loss(pred, target):\n",
"    return F.mse_loss(torch.tanh(pred.float()), target)\n","\n",
"m = PelancaNNUEv3()\n","print(f'Params: {sum(p.numel() for p in m.parameters()):,}')\n","del m"
]

# Cell 10: Export V3
nb['cells'][10]['source'] = [
"def export_nnue_v3(model, path, verbose=True):\n",
"    model.eval()\n","    m = model.module if hasattr(model, 'module') else model\n",
"    with open(path, 'wb') as f:\n",
"        f.write(b'PLNN')\n","        f.write(struct.pack('<I', 3))\n",
"        f.write(struct.pack('<I', INPUT_SIZE))\n",
"        f.write(struct.pack('<I', FT_SIZE))\n",
"        f.write(struct.pack('<I', HIDDEN_SIZE))\n",
"        ft_w = (m.ft.weight.data.cpu().T * FT_QUANT).round().clamp(-32767, 32767).to(torch.int16)\n",
"        ft_b = (m.ft_bias.data.cpu() * FT_QUANT).round().clamp(-32767, 32767).to(torch.int16)\n",
"        f.write(ft_w.contiguous().numpy().tobytes())\n",
"        f.write(ft_b.numpy().tobytes())\n",
"        h_w = (m.hidden.weight.data.cpu() * HIDDEN_QUANT).round().clamp(-127, 127).to(torch.int8)\n",
"        h_b = (m.hidden.bias.data.cpu() * FT_QUANT * HIDDEN_QUANT).round().clamp(-2**30, 2**30).to(torch.int32)\n",
"        f.write(h_w.numpy().tobytes())\n","        f.write(h_b.numpy().tobytes())\n",
"        o_w = (m.out.weight.data.cpu() * OUTPUT_QUANT).round().clamp(-127, 127).to(torch.int8)\n",
"        o_b = (m.out.bias.data.cpu() * FT_QUANT * HIDDEN_QUANT * OUTPUT_QUANT).round().clamp(-2**30, 2**30).to(torch.int32)\n",
"        f.write(o_w.numpy().tobytes())\n","        f.write(o_b.numpy().tobytes())\n",
"    if verbose:\n","        sz = os.path.getsize(path)\n",
"        print(f'Exportado: {path} ({sz:,} bytes)')\n","\n","print('Export v3 OK')"
]

# Cell 12: Load
nb['cells'][12]['source'] = [
"import gc; gc.collect()\n","print('Carregando...\\n')\n",
"train_ds = NnueSparseDatasetV3(HDF5_FILE, 'train', mirror=MIRROR_AUG)\n","gc.collect()\n",
"val_ds = NnueSparseDatasetV3(HDF5_FILE, 'val', max_pos=500_000)\n","gc.collect()\n",
"VAL_BATCH = min(BATCH_SIZE, len(val_ds))\n",
"effective = len(train_ds)\n","n_batches = effective // BATCH_SIZE\n",
"print(f'\\nTrain: {n_batches} batches x {BATCH_SIZE:,} ({effective:,} eff)')\n",
"print(f'Gradient steps total: {n_batches * EPOCHS:,}')"
]

# Cell 14: Training (5 values, no WDL)
nb['cells'][14]['source'] = [
"device = torch.device('cuda' if torch.cuda.is_available() else 'cpu')\n",
"use_amp = torch.cuda.is_available()\n",
"model = PelancaNNUEv3().to(device)\n",
"optimizer = optim.Adam(model.parameters(), lr=LR_MAX, weight_decay=WEIGHT_DECAY)\n",
"scheduler = optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=EPOCHS, eta_min=LR_MIN)\n",
"scaler = torch.amp.GradScaler('cuda') if use_amp else None\n",
"best_val = float('inf')\n","hist = {'train': [], 'val': [], 'lr': []}\n",
"print(f'GPU: {torch.cuda.get_device_name(0)}')\n",
"print(f'Treino: {EPOCHS} epochs | LR: {LR_MAX} -> {LR_MIN} | MSE puro')\n",
"print(f'Batches/epoch: {len(train_ds) // BATCH_SIZE}\\n')\n",
"for epoch in range(EPOCHS):\n","    t0 = time.time()\n",
"    model.train()\n","    tl, tn = 0.0, 0\n",
"    for stm_idx, stm_off, nstm_idx, nstm_off, ev in make_batches(train_ds, BATCH_SIZE, shuffle=True):\n",
"        stm_idx = stm_idx.to(device, non_blocking=True)\n",
"        stm_off = stm_off.to(device, non_blocking=True)\n",
"        nstm_idx = nstm_idx.to(device, non_blocking=True)\n",
"        nstm_off = nstm_off.to(device, non_blocking=True)\n",
"        ev = ev.to(device, non_blocking=True).unsqueeze(1)\n",
"        optimizer.zero_grad(set_to_none=True)\n",
"        if use_amp:\n","            with torch.amp.autocast('cuda'):\n",
"                loss = nnue_loss(model(stm_idx, stm_off, nstm_idx, nstm_off), ev)\n",
"            scaler.scale(loss).backward()\n","            scaler.unscale_(optimizer)\n",
"            nn.utils.clip_grad_norm_(model.parameters(), 1.0)\n",
"            scaler.step(optimizer)\n","            scaler.update()\n",
"        else:\n","            loss = nnue_loss(model(stm_idx, stm_off, nstm_idx, nstm_off), ev)\n",
"            loss.backward()\n","            nn.utils.clip_grad_norm_(model.parameters(), 1.0)\n",
"            optimizer.step()\n","        tl += loss.item(); tn += 1\n",
"    train_loss = tl / max(tn, 1)\n",
"    model.eval()\n","    vl, vn = 0.0, 0\n","    with torch.no_grad():\n",
"        for stm_idx, stm_off, nstm_idx, nstm_off, ev in make_batches(val_ds, VAL_BATCH, shuffle=False):\n",
"            stm_idx = stm_idx.to(device)\n","            stm_off = stm_off.to(device)\n",
"            nstm_idx = nstm_idx.to(device)\n","            nstm_off = nstm_off.to(device)\n",
"            ev = ev.to(device).unsqueeze(1)\n",
"            loss = nnue_loss(model(stm_idx, stm_off, nstm_idx, nstm_off), ev)\n",
"            vl += loss.item(); vn += 1\n",
"    val_loss = vl / max(vn, 1)\n","    scheduler.step()\n",
"    lr = optimizer.param_groups[0]['lr']\n",
"    hist['train'].append(train_loss)\n","    hist['val'].append(val_loss)\n",
"    hist['lr'].append(lr)\n","    mk = ''\n",
"    if val_loss < best_val:\n","        best_val = val_loss\n",
"        torch.save({'epoch': epoch, 'model': model.state_dict(), 'val': val_loss},\n",
"                   os.path.join(CKPT_DIR, 'best.pt'))\n","        mk = ' ** BEST'\n",
"    elapsed = time.time() - t0\n",
"    print(f'E{epoch:3d} | t={train_loss:.6f} v={val_loss:.6f} lr={lr:.1e} | {elapsed:.0f}s{mk}')\n",
"    if (epoch + 1) % 15 == 0:\n",
"        p = os.path.join(CKPT_DIR, f'e{epoch}.nnue')\n",
"        export_nnue_v3(model, p, verbose=False)\n",
"        print(f'  -> {p}')\n",
"print(f'\\nMelhor val: {best_val:.6f}')"
]

# Cell 18: Final export
nb['cells'][18]['source'] = [
"ck = torch.load(os.path.join(CKPT_DIR, 'best.pt'), map_location='cpu')\n",
"fm = PelancaNNUEv3()\n","fm.load_state_dict(ck['model'])\n",
"print(f'Best: epoch {ck[\"epoch\"]}, val={ck[\"val\"]:.6f}\\n')\n",
"export_nnue_v3(fm, NNUE_OUTPUT)\n",
"print(f'\\nPRONTO: {NNUE_OUTPUT} ({os.path.getsize(NNUE_OUTPUT):,} bytes)')"
]

json.dump(nb, open('D:/pelanca/training/pelanca_nnue_v2_kaggle.ipynb', 'w'), indent=1)
print('Notebook v3 salvo!')
