import pathlib,sys,re,subprocess,json,difflib
platform=sys.platform
P='crates/maestro-catalog/src/policy/workspace/paths.rs'
U='crates/maestro-filesystem/src/unix_creation.rs'
W='crates/maestro-filesystem/src/windows.rs'
S='crates/maestro-filesystem/src/windows_security.rs'
V='crates/maestro-filesystem/src/publication.rs'
F='files::tests::workspace_trust::effects::'
cases=[
 ('source-open-checkpoint',V,'fn publish_verified_inner','(checks.after_source_open)()?;','/* skip */','maestro-catalog',F+'publication_after_source_open_rechecks_deny_before_reading_or_linking'),
 ('before-link-checkpoint',V,'fn publish_verified_inner','(checks.before_link)()?;','/* skip */','maestro-filesystem','publication_checkpoints_run_in_named_open_before_after_order'),
 ('after-link-checkpoint',V,'fn publish_verified_inner','(checks.after_link)()','Ok::<(), std::io::Error>(())','maestro-catalog',F+'publication_rechecks_both_paths_and_rolls_back_only_the_created_link'),
 ('read-identity',V,'fn publish_verified_inner','self.verify_created(from, &source)?;','/* skip */','maestro-filesystem','publication_identity_is_rechecked_after_source_open_before_read_checkpoint'),
 ('post-link-identity',V,'fn publish_verified_inner','(checks.after_link)().and_then(|()| self.verify_created(to, &source))','(checks.after_link)()','maestro-filesystem','publication_post_link_compare_refuses_source_swap_in_the_last_syscall_window'),
 ('swap-checkpoints',V,'fn publish_verified_inner','(checks.after_source_open)()?; self.verify_created(from, &source)?; let mut bytes = Vec::new(); source.read_to_end(&mut bytes)?; (checks.before_link)()?;','(checks.before_link)()?; self.verify_created(from, &source)?; let mut bytes = Vec::new(); source.read_to_end(&mut bytes)?; (checks.after_source_open)()?;','maestro-filesystem','publication_checkpoints_run_in_named_open_before_after_order'),
 ('source-identity',V,'fn publish_verified_inner','source.read_to_end(&mut bytes)?; (checks.before_link)()?; self.verify_created(from, &source)?;','source.read_to_end(&mut bytes)?; (checks.before_link)()?; /* skip */','maestro-catalog',F+'publication_refuses_regular_source_swap_after_held_read_before_link'),
 ('source-symlink-identity',V,'fn publish_verified_inner','source.read_to_end(&mut bytes)?; (checks.before_link)()?; self.verify_created(from, &source)?;','source.read_to_end(&mut bytes)?; (checks.before_link)()?; /* skip */','maestro-catalog',F+'publication_refuses_symlink_source_swap_after_held_read_before_link'),
 ('published-replacement-identity',V,'fn publish_verified_inner','(checks.after_link)().and_then(|()| self.verify_created(to, &source))','(checks.after_link)()','maestro-catalog',F+'publication_post_compare_preserves_a_replacement_of_the_published_link'),
 ('mkdir-post-floor',P,'fn create_directory_with','.check_current_policy()','.parent.verify_named()','maestro-catalog',F+'deny_alias_rebinding_on_both_sides_of_mkdir_and_write_leaves_no_new_entries'),
 ('write-post-floor',P,'fn write_new_with','result .and_then(|()| self.check_current_policy())','result','maestro-catalog',F+'deny_alias_rebinding_on_both_sides_of_mkdir_and_write_leaves_no_new_entries'),
 ('removal-post-floor',P,'fn remove_verified_with','self.check_current_policy()','Ok(())','maestro-catalog',F+'removal_restores_quarantined_bytes_when_post_effect_policy_refuses'),
]
if platform=='win32':
 cases += [
 ('native-directory-disposition',S,'fn remove_created_directory','DeleteFile: true','DeleteFile: false','maestro-catalog',F+'deny_alias_rebinding_on_both_sides_of_mkdir_and_write_leaves_no_new_entries'),
 ('directory-delete-share-leak',P,'fn create_directory_with','Ok(hardened) => Ok(hardened)','Ok(_hardened) => Ok(child)','maestro-catalog',F+'newly_created_parent_handles_block_rename_before_reuse'),
 ('hardened-directory-identity',W,'pub fn harden_created_child','self.verify_created(name, held)?;','/* skip */','maestro-catalog',F+'directory_hardening_refuses_replacement_before_returning_a_parent_grant'),
 ('rollback-full-identity',S,'fn remove_created_directory','if !same_file(&file, created)? {','if false {','maestro-catalog',F+'directory_rollback_restores_a_replacement_instead_of_deleting_it'),
 ('created-full-identity',W,'pub fn verify_created','if !same_file(created, &named)? {','if false {','maestro-catalog',F+'written_bytes_are_not_owned_when_the_created_name_is_replaced'),
 ('created-no-follow',W,'pub fn verify_created','hold(&self.path.join(name), OPEN_REPARSE_DIRECTORY_FLAGS)','hold(&self.path.join(name), FILE_FLAG_BACKUP_SEMANTICS)','maestro-filesystem','created_identity_matches_only_regular_same_objects_never_a_symlink_alias'),
 ('rollback-written-identity',W,'fn remove_verified_impl','&& !same_file(created, &file)?','&& false','maestro-catalog',F+'written_file_rollback_preserves_a_replacement_with_identical_bytes'),
 ]
else:
 cases += [
 ('hardened-directory-identity',U,'pub fn harden_created_child','self.verify_created(name, &created.0)?;','/* skip */','maestro-catalog',F+'directory_hardening_refuses_replacement_before_returning_a_parent_grant'),
 ('rollback-full-identity',U,'pub fn remove_created_child','self.verify_created(&quarantine, &created.0)?;','/* skip */','maestro-catalog',F+'directory_rollback_restores_a_replacement_instead_of_deleting_it'),
 ('created-full-identity',U,'pub fn verify_created','if named.st_dev != held.st_dev || named.st_ino != held.st_ino {','if false {','maestro-catalog',F+'written_bytes_are_not_owned_when_the_created_name_is_replaced'),
 ('created-no-follow',U,'pub fn verify_created','AtFlags::SYMLINK_NOFOLLOW','AtFlags::empty()','maestro-filesystem','created_identity_matches_only_regular_same_objects_never_a_symlink_alias'),
 ('rollback-written-identity',U,'fn remove_created_inner','self.verify_created(quarantine, created)?;','/* skip */','maestro-catalog',F+'written_file_rollback_preserves_a_replacement_with_identical_bytes'),
 ]
output=pathlib.Path('native-proof');output.mkdir(exist_ok=True)
results=[]
for name,path,anchor,before,after,package,test in cases:
 file=pathlib.Path(path);original=file.read_text();begin=original.index(anchor);tail=original[begin:]
 pattern=r'\s*'.join(re.escape(word) for word in before.split());match=re.search(pattern,tail)
 if match is None: raise RuntimeError(f'{name}: mutation not found: {before}')
 changed=original[:begin]+tail[:match.start()]+after+tail[match.end():]
 (output/(name+'.diff')).write_text(''.join(difflib.unified_diff(original.splitlines(True),changed.splitlines(True),fromfile=path,tofile=path)))
 command=['cargo','test','--locked','-p',package,test.split('::')[-1],'--','--exact' ]
 # Full source-qualified test name avoids zero-test success, including module hierarchy.
 if package=='maestro-catalog': command[5]=test
 else: command[5]='created_identity_tests::'+test
 file.write_text(changed)
 try:
  result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 finally: file.write_text(original)
 (output/(name+'.log')).write_text(result.stdout)
 killed=result.returncode==101 and 'test result: FAILED' in result.stdout and '1 failed' in result.stdout
 results.append(dict(name=name,command=command,exit=result.returncode,killed=killed))
 print(name,'KILLED' if killed else 'NOT KILLED',result.returncode,flush=True)
 (output/'results.json').write_text(json.dumps(results,indent=2))
 if not killed: print(result.stdout);sys.exit(1)
print('TOTAL',len(results),'KILLED',sum(r['killed'] for r in results),flush=True)
