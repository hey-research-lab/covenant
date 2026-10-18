#!/usr/bin/env python3
"""Protected structural acceptance. Solver proof remains a separate paid gate."""
import hashlib,json,os,pathlib,re
ROOT=pathlib.Path.cwd()
CONTRACT=json.loads('{"base_commit":"81fabc86869d0a4db7da3fabc7eab5430b1f02c5","base_tree":"6b2882d92ffad6e8df932acae4a0a3d356a1d372","proof_path":"tests/proofs_v16.rs","proof_prefix_bytes":790518,"proof_prefix_sha256":"391ed9a4b32aa60f9acbae63afe08be5d357714a86d768ca1264401c6a9e7ea2","unchanged_files":{".cargo/config.toml":"42ed95bb6403adff247ac21da9773c72d47ebd320c09828f27d7954d3a12d4a5",".gitignore":"e155305dfe798f4020e2469cbc18b34d8daa42fcef4fe24c5b5b64c6daa9ac2b","Cargo.lock":"189914a39b7a41adfae9493479145e2f8d78d4e1f76e114766d433c274a3a7a7","Cargo.toml":"26cf3e9510aa9192b97c7244d6c61b29d919aa7fa216d29a166eef2e48e973b6","LICENSE":"c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4","README.md":"a28fcd28672403904dfa79180b3c7e148a9d29f620a6af6afc9b47bf372fa113","kani-list.json":"eb86538b68c748d3e0a69e962bff3d7eda99978724026f9868b323f2d52aee5d","scripts/contracts_runner.sh":"ed4e612f11ecf14cebea6a79ffe4f34a8a6c6c81fae18963bff8239ad12ad1d1","scripts/isolated_full_audit.sh":"e542648400b15f23fe4c2a36db852b1636824c14e0d664bf66cb247781e3a926","scripts/isolated_retry.sh":"4469931c0ebddac5ea20bfdb89b83af7951b1abac494c4067b127a4acab4143f","scripts/isolated_runner.sh":"de6852c6f54dd5e08835080b17cb3572b039cb3b8274e1f0c484a6c5e610558c","scripts/kani_audit_certified.tsv":"6a43cbac67c51514240758a84ce36a529f477f5c88b5e03abbbfcde350d490a6","scripts/kernel_results.tsv":"f04fb99ca1263ebf1acc40f38747568d5d45732e1238e454a7e570489da3cc80","spec.md":"4cdf25b5e3c68b5cc3ca7dcf42a14e7666ea9792e96089a430937e6c670a756b","src/lib.rs":"e7c468cf4b8613b17890fbf2ff371990a021729584a26ec2e2aee4221061f700","src/v16.rs":"3d0c68703b488e897a4c8beba51a16cd1e647e74521407176c7daf6a7401b37d","src/v16_kani_api.rs":"ce11affb4dfd703a2225a39230636c549427972f50aeaad5ce23d90051a24ff2","src/v16_proofs.rs":"1436de1ae5d4204d7b9afb6f25f4ded24bf4271edee8546a6b9c3000ed643b54","src/wide_math.rs":"5c3c47f5d99cb0dd986d8abb59fa16e2a2f03c0a26b80bca5c27242ef0281872","tests/backing_double_claim_fuzz.proptest-regressions":"88cc1d2bbc939e0b5106129860bb055d4617b44e3f9044f45ffa13c4a86879cb","tests/backing_double_claim_fuzz.rs":"90827f0d4e6ff663a9df8d1fdf6629f0048ae686a1e99e82fb8bc87dbfdc5f05","tests/proofs_v16_arithmetic.rs":"3626844dc54d0f6548b4e1f48ca7d51d76c1e2cf71e19b5573ea4288df4a5e2b","tests/reference_model_conformance.rs":"9bb527b50235894855bfaca97684cd2c64ece4be14cacc6afb71f60648c1d5c9","tests/resolved_insolvent_fuzz.rs":"a7aac5cb647bb6f423029f672f77dea311084784e39b83b3ae1d095cd69c308e","tests/rounding_residue_fuzz.proptest-regressions":"611d3e8a90eb96d98f00b3c0ee1d20ebe36881cf5b040bfb2c85d927737615c5","tests/rounding_residue_fuzz.rs":"9a4433f183ef05a53fa8136ee09ba8f9b8e25946b44e1621a1cda03d2a35fdcd","tests/v16_fuzzing.rs":"d1e061a954a343e85b83e3af3ff71b57c9bb35b5d3161c745ba2618010b17d59","tests/v16_spec_tests.rs":"0b242532c96b9fb445aae0f6c85b5c81f0075b5ae6609168f8d8716df17f4c54"},"harnesses":["proof_v16_public_forfeit_long_flat_residue","proof_v16_public_auto_crank_long_residue"],"assertion_labels":["PR239_FORFEIT_LONG_FLAT_RELEASE_INVARIANTS","PR239_AUTO_CRANK_LONG_RELEASE_INVARIANTS"],"cover_labels":["pr239 forfeit zero residue","pr239 forfeit long-only residue","pr239 forfeit short-only residue","pr239 forfeit two-sided residue","pr239 auto-crank zero residue","pr239 auto-crank two-sided residue"],"allowed_new_files":["docs/proofs/pr239-route-domains.md"]}')
def require(ok,message):
    if not ok:raise SystemExit('PR239 authoring acceptance: '+message)
def check_inventory():
    # Checker Git metadata is intentionally outside the mounted source tree.
    # Inspect the complete filesystem, including files already staged by git apply.
    allowed=set(CONTRACT['unchanged_files']) | {CONTRACT['proof_path'], '.covenant-pr239/acceptance.py'} | set(CONTRACT['allowed_new_files'])
    allowed_dirs={str(parent) for name in allowed for parent in pathlib.PurePosixPath(name).parents if str(parent)!='.'}
    for current,dirs,files in os.walk(ROOT,followlinks=False):
        for name in list(dirs):
            p=pathlib.Path(current)/name
            relative=p.relative_to(ROOT).as_posix()
            require(not p.is_symlink(),'symlinked source directory '+relative)
            if relative=='target':
                dirs.remove(name) # Generated Rust build output is not authored source.
            else:
                require(relative in allowed_dirs,'unexpected source directory '+relative)
        for name in files:
            p=pathlib.Path(current)/name
            relative=p.relative_to(ROOT).as_posix()
            require(p.is_file() and not p.is_symlink(),'nonregular source file '+relative)
            require(relative in allowed,'unexpected source file '+relative)
check_inventory()
for name,expected in CONTRACT['unchanged_files'].items():
    p=ROOT/name
    require(p.is_file() and not p.is_symlink(), 'removed or replaced protected file '+name)
    require(hashlib.sha256(p.read_bytes()).hexdigest()==expected,'changed protected file '+name)
proof=(ROOT/CONTRACT['proof_path']).read_bytes()
n=CONTRACT['proof_prefix_bytes']
require(len(proof)>n,'new proof appendix is missing')
require(hashlib.sha256(proof[:n]).hexdigest()==CONTRACT['proof_prefix_sha256'],'existing proof prefix changed')
appendix=proof[n:].decode('utf-8')
require(len(appendix.encode())<=48*1024,'proof appendix exceeds 48KiB')
for name in CONTRACT['harnesses']:
    require(len(re.findall(r'fn\s+'+re.escape(name)+r'\s*\(',appendix))==1,'missing/duplicate harness '+name)
require(len(re.findall(r'#\[kani::proof\]',appendix))==2,'need exactly two proof entrypoints')
require('forfeit_recovery_leg_not_atomic' in appendix,'public forfeit route missing')
require('permissionless_auto_crank_not_atomic' in appendix,'public auto-crank route missing')
require(len(re.findall(r'kani::any',appendix))>=3,'residue inputs must remain symbolic')
for label in CONTRACT['assertion_labels']+CONTRACT['cover_labels']:
    require('"'+label+'"' in appendix,'required assertion/cover label missing: '+label)
for pattern in [r'kani::stub',r'kani::assume\s*\(\s*false',r'kani::assume\s*\(\s*(?:result|after)\b',r'cfg\s*\(\s*not\s*\(\s*kani']:
    require(not re.search(pattern,appendix),'forbidden stub, vacuous assumption or disabled Kani route')
for name in CONTRACT['allowed_new_files']:
    p=ROOT/name
    require(p.is_file() and not p.is_symlink(),'domain document missing')
    text=p.read_text()
    for harness in CONTRACT['harnesses']:require(harness in text,'domain document omits '+harness)
    require(all(word in text.lower() for word in ['narrow','symbolic','0..=3','not yet']), 'document must state narrowed domain and unverified status')
print('PR239 authoring contract passed: unchanged source/prefix; two narrowed route harnesses present. Kani compile and green/red verdicts are NOT established by this check.')
