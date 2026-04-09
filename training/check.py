import json, sys

nb = json.load(open('D:/pelanca/training/pelanca_nnue_v2_kaggle.ipynb'))
errs = []

for i, cell in enumerate(nb['cells']):
    src = ''.join(cell['source'])

    if i == 6:
        if 'nn.EmbeddingBag' in src: errs.append(f'C6: EmbeddingBag no dataset (deveria estar no modelo)')
        if 'def nnue_collate' not in src: errs.append(f'C6: nnue_collate nao definido')
        if 'return w_idx, b_idx, ev, wdl' not in src: errs.append(f'C6: __getitem__ return errado')
        if 'for w_idx, b_idx, ev, wdl in batch' not in src: errs.append(f'C6: collate unpack errado')
        if 'astype(np.int64)' not in src: errs.append(f'C6: falta conversao int16->int64')
        if 'torch.zeros' in src: errs.append(f'C6: torch.zeros no dataset (gargalo!)')
        if '^ 7' not in src: errs.append(f'C6: mirror XOR 7 ausente')
        if 'w_idx, b_idx = b_idx, w_idx' not in src: errs.append(f'C6: STM swap ausente')
        if 'dtype=np.int16' not in src: errs.append(f'C6: nao usa int16 para storage')

    if i == 8:
        if 'nn.EmbeddingBag' not in src: errs.append(f'C8: modelo sem EmbeddingBag')
        if 'ft_bias' not in src: errs.append(f'C8: ft_bias nao definido')
        if 'def forward(self, stm_idx, stm_off, nstm_idx, nstm_off)' not in src: errs.append(f'C8: forward sig errada')
        if 'wdl_blended_loss' not in src: errs.append(f'C8: loss nao definida')

    if i == 10:
        if '.T' not in src: errs.append(f'C10: export nao transpoe EmbeddingBag weight')
        if 'ft_bias' not in src: errs.append(f'C10: export nao salva ft_bias')
        # Check version 2
        if "2)" not in src: errs.append(f'C10: version nao e 2')

    if i == 12:
        if 'collate_fn=nnue_collate' not in src: errs.append(f'C12: DataLoader sem collate_fn')

    if i == 14:
        if 'stm_idx, stm_off, nstm_idx, nstm_off, ev, wdl in train_ld' not in src: errs.append(f'C14: train unpack errado')
        if 'stm_idx, stm_off, nstm_idx, nstm_off, ev, wdl in val_ld' not in src: errs.append(f'C14: val unpack errado')
        if 'model(stm_idx, stm_off, nstm_idx, nstm_off)' not in src: errs.append(f'C14: model call errado')
        if 'nn.DataParallel(' in src: errs.append(f'C14: DataParallel ATIVO')
        if 'torch.cuda.amp.autocast' in src: errs.append(f'C14: autocast API antiga')
        if 'torch.cuda.amp.GradScaler' in src: errs.append(f'C14: GradScaler API antiga')
        if "torch.amp.autocast('cuda')" not in src: errs.append(f'C14: falta torch.amp.autocast')
        if "torch.amp.GradScaler('cuda')" not in src: errs.append(f'C14: falta torch.amp.GradScaler')

# Cross-cell: variable flow
all_src = '\n'.join(''.join(c['source']) for c in nb['cells'])
for var in ['INPUT_SIZE', 'FT_SIZE', 'HIDDEN_SIZE', 'BATCH_SIZE', 'EPOCHS',
            'LR_MAX', 'WDL_WEIGHT', 'HDF5_FILE', 'CKPT_DIR', 'NNUE_OUTPUT',
            'FT_QUANT', 'HIDDEN_QUANT', 'OUTPUT_QUANT', 'MIRROR_AUG']:
    if f'{var} =' not in all_src:
        errs.append(f'Variavel {var} nunca definida')

print(f'=== RESULTADO: {len(errs)} erros ===')
for e in errs:
    print(f'  X {e}')
if not errs:
    print('  ZERO ERROS - notebook pronto!')
