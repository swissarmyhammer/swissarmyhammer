# Use Cases and Examples

SwissArmyHammer adapts to various development workflows. Here are practical scenarios showing how to use it effectively.

## Individual Developer

### Personal Prompt Library
Create reusable prompts for consistent code quality:

```bash
# Set up personal collection
mkdir -p ~/.swissarmyhammer/prompts

# Code review checklist
cat > ~/.swissarmyhammer/prompts/self-review.md << 'EOF'
---
title: Self Code Review
description: Personal code review checklist
arguments:
  - name: language
    description: Programming language
    required: true
  - name: feature
    description: Feature being developed
    required: true
---

# Self Review: {{feature}} ({{language}})

Review this {{feature}} implementation:

## Quality Check
- [ ] Code follows language conventions
- [ ] Functions are documented
- [ ] Error handling is comprehensive
- [ ] Tests cover main scenarios

## Security Review
- [ ] Input validation implemented
- [ ] No hardcoded secrets
- [ ] Authentication/authorization proper

Please analyze my {{feature}} code and provide specific feedback.
EOF

# Use in Claude Code
/self-review language="rust" feature="authentication"
```

### Development Workflow
Automate your development process:

```markdown
---
name: feature-workflow
description: Complete feature development process
initial_state: plan
---

### plan
Plan feature implementation
**Actions**: prompt planning, memo documentation
**Next**: implement

### implement  
Write feature code
**Actions**: issue creation, branch switching
**Next**: review

### review
Self-review implementation  
**Actions**: self-review prompt, test execution
**Transitions**: If issues → implement, If good → complete
```

## Small Team (3-5 developers)

### Team Standards
Shared prompts in project repository:

```bash
# Team code review standard
mkdir -p .swissarmyhammer/prompts
cat > .swissarmyhammer/prompts/team-review.md << 'EOF'
---
title: Team Code Review
description: Standardized review process
arguments:
  - name: author
    required: true
  - name: urgency
    choices: ["low", "medium", "high", "critical"]
    default: "medium"
---

# Code Review - {{author}}

**Priority**: {{urgency | upcase}}

## Team Checklist
- [ ] Follows linting rules
- [ ] Unit tests included
- [ ] Documentation updated
- [ ] No security issues

{% if urgency == "critical" %}
## CRITICAL REVIEW
Focus on correctness and no regressions.
{% endif %}

Provide specific feedback on each item.
EOF
```

### Pull Request Workflow
```markdown
---
name: pr-workflow
description: Team PR process
initial_state: create_pr
---

### create_pr
Create pull request
**Actions**: git push, PR creation, reviewer assignment
**Next**: ci_checks

### ci_checks  
Run automated checks
**Transitions**: Pass → review, Fail → fix_tests

### review
Team code review
**Actions**: team-review prompt execution
**Transitions**: Approved → merge, Changes needed → fix_feedback
```

## Large Team/Enterprise

### Compliance Review
Enterprise-grade architecture review:

```markdown
---
title: Enterprise Architecture Review
description: Compliance and security review
arguments:
  - name: system_name
    required: true
  - name: compliance_level
    choices: ["standard", "regulated", "highly-regulated"]
    default: "standard"
---

# Architecture Review: {{system_name}}

**Compliance**: {{compliance_level | title}}

## Security Requirements
{% if compliance_level == "highly-regulated" %}
- [ ] End-to-end encryption
- [ ] Multi-factor authentication
- [ ] Zero-trust architecture
{% else %}
- [ ] Basic authentication
- [ ] HTTPS enforcement
- [ ] Input validation
{% endif %}

## Data Protection
- [ ] PII handling procedures
- [ ] Data retention policies
- [ ] Backup and recovery tested
```

### Release Workflow
```markdown
---
name: enterprise-release  
description: Enterprise release process
initial_state: architecture_review
---

### architecture_review
Enterprise architecture review
**Actions**: compliance review, security scan
**Transitions**: Pass → approvals, Fail → remediation

### approvals
Required stakeholder approvals
**Actions**: business approval, security approval, compliance approval
**Next**: production_deploy

### production_deploy
Deploy with monitoring
**Actions**: deployment, health checks, notification
```

## Industry-Specific Examples

### Financial Services
```markdown
---
title: Financial Compliance Review
description: Financial regulatory compliance
arguments:
  - name: regulation
    choices: ["PCI-DSS", "SOX", "GDPR"]
    required: true
---

# {{regulation}} Compliance Review

{% case regulation %}
{% when "PCI-DSS" %}
## Payment Card Industry Requirements
- [ ] Cardholder data encryption
- [ ] Network segmentation
- [ ] Access controls
{% when "SOX" %}
## Sarbanes-Oxley Requirements  
- [ ] Internal controls documented
- [ ] Change management enforced
- [ ] Data integrity controls
{% endcase %}

Please assess compliance and provide remediation steps.
```

### Healthcare (HIPAA)
```markdown
---
title: HIPAA Compliance Review
description: Healthcare data protection review
arguments:
  - name: phi_types
    description: Types of PHI handled
    type: array
    required: true
---

# HIPAA Compliance Review

**PHI Types**: {{phi_types | join: ", "}}

## Technical Safeguards
- [ ] Unique user identification
- [ ] Access controls implemented
- [ ] Audit logs comprehensive
- [ ] Transmission security

## Physical Safeguards
- [ ] Facility access controls
- [ ] Workstation security
- [ ] Device controls

Provide detailed compliance assessment.
```

## Open Source Project

### Project Health Assessment
```markdown
---
title: OSS Project Health
description: Open source project assessment
arguments:
  - name: project_name
    required: true
  - name: contributors
    type: number
    required: true
---

# Project Health: {{project_name}}

**Contributors**: {{contributors}}

## Community Health
- [ ] Contributor onboarding documented
- [ ] Code of conduct present
- [ ] Good first issues available

{% if contributors > 20 %}
## Large Project Requirements
- [ ] Governance structure defined
- [ ] Decision-making transparent
- [ ] Regular maintainer meetings
{% else %}
## Small Project Requirements
- [ ] Primary maintainer identified
- [ ] Basic contribution workflow
- [ ] Backup maintainer designated
{% endif %}

## Technical Health
- [ ] Automated testing (>80% coverage)
- [ ] CI/CD configured
- [ ] Security scanning enabled

Assess current state and provide improvement recommendations.
```

### Release Process
```markdown
---
name: oss-release
description: Open source release workflow
initial_state: health_check
---

### health_check
Assess project health
**Actions**: health assessment, community metrics
**Next**: version_prep

### version_prep
Prepare version bump
**Actions**: changelog generation, version update
**Next**: testing

### testing
Comprehensive test suite
**Actions**: test matrix, integration tests, security scan
**Next**: community_review

### community_review
Community feedback period
**Actions**: release PR, announcement, 48-hour wait
**Next**: release

### release
Publish release
**Actions**: artifact building, package publishing, announcement
```

## Development Team Scenarios

### Code Review Automation
```bash
# Automated code review workflow
sah flow run code-review-workflow \
  --var author="john.doe" \
  --var files="src/auth.rs,src/db.rs" \
  --var priority="high"
```

### Issue Resolution
```bash
# Complete issue workflow
sah issue create "Fix login timeout"
sah issue work ISSUE-456
# ... implement fix ...
sah flow run fix-validation
sah issue complete ISSUE-456
```

### Documentation Generation
```bash
# Generate project documentation
sah prompt test docs/project \
  --var project_name="MyApp" \
  --var language="rust"

# Run documentation review workflow
sah flow run doc-review-workflow
```

These examples show SwissArmyHammer's flexibility across different team sizes, industries, and use cases while maintaining consistency and quality.