#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branches {
    branches: std::collections::BTreeMap<git2::Oid, Vec<crate::repo::Branch>>,
}

impl Branches {
    pub fn new(branches: impl Iterator<Item = crate::repo::Branch>) -> Self {
        let mut grouped_branches = std::collections::BTreeMap::new();
        for branch in branches {
            grouped_branches
                .entry(branch.id)
                .or_insert_with(|| Vec::new())
                .push(branch);
        }
        Self {
            branches: grouped_branches,
        }
    }

    pub fn contains_oid(&self, oid: git2::Oid) -> bool {
        self.branches.contains_key(&oid)
    }

    pub fn get(&self, oid: git2::Oid) -> Option<&[crate::repo::Branch]> {
        self.branches.get(&oid).map(|v| v.as_slice())
    }

    pub fn remove(&mut self, oid: git2::Oid) -> Option<Vec<crate::repo::Branch>> {
        self.branches.remove(&oid)
    }

    pub fn oids(&self) -> impl Iterator<Item = git2::Oid> + '_ {
        self.branches.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (git2::Oid, &[crate::repo::Branch])> + '_ {
        self.branches
            .iter()
            .map(|(oid, branch)| (*oid, branch.as_slice()))
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn all(&self) -> Self {
        self.clone()
    }

    pub fn dependents(
        &self,
        repo: &dyn crate::repo::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
    ) -> Self {
        let branches = self
            .branches
            .iter()
            .filter(|(branch_oid, branch)| {
                let is_shared_base = repo
                    .merge_base(**branch_oid, head_oid)
                    .map(|merge_oid| merge_oid == base_oid && **branch_oid != base_oid)
                    .unwrap_or(false);
                let is_base_descendant = repo
                    .merge_base(**branch_oid, base_oid)
                    .map(|merge_oid| merge_oid == base_oid)
                    .unwrap_or(false);
                if is_shared_base {
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        branch_name,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        branch_name,
                        base_oid
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.iter().cloned().collect();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn branch(
        &self,
        repo: &dyn crate::repo::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
    ) -> Self {
        let branches = self
            .branches
            .iter()
            .filter(|(branch_oid, branch)| {
                let is_head_ancestor = repo
                    .merge_base(**branch_oid, head_oid)
                    .map(|merge_oid| **branch_oid == merge_oid)
                    .unwrap_or(false);
                let is_base_descendant = repo
                    .merge_base(**branch_oid, base_oid)
                    .map(|merge_oid| merge_oid == base_oid)
                    .unwrap_or(false);
                if !is_head_ancestor {
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        branch_name,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        branch_name,
                        base_oid
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.iter().cloned().collect();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn protected(&self, protected: &crate::protect::ProtectedBranches) -> Self {
        let branches: std::collections::BTreeMap<_, _> = self
            .branches
            .iter()
            .filter_map(|(oid, branches)| {
                let protected_branches: Vec<_> = branches
                    .iter()
                    .filter_map(|b| {
                        if protected.is_protected(&b.name) {
                            log::trace!("Branch {} is protected", b.name);
                            Some(b.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if protected_branches.is_empty() {
                    None
                } else {
                    Some((*oid, protected_branches))
                }
            })
            .collect();

        Self { branches }
    }
}

pub fn find_protected_base<'b>(
    repo: &dyn crate::repo::Repo,
    protected_branches: &'b Branches,
    head_oid: git2::Oid,
) -> Option<&'b crate::repo::Branch> {
    let protected_base_oids: std::collections::HashMap<_, _> = protected_branches
        .oids()
        .filter_map(|oid| {
            repo.merge_base(head_oid, oid).map(|merge_oid| {
                (
                    merge_oid,
                    protected_branches.get(oid).expect("oid is known to exist"),
                )
            })
        })
        .collect();
    repo.commits_from(head_oid)
        .filter_map(|commit| {
            if let Some(branches) = protected_base_oids.get(&commit.id) {
                Some(
                    branches
                        .first()
                        .expect("there should always be at least one"),
                )
            } else {
                None
            }
        })
        .next()
}
