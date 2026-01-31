Code, Documentation, and Testing Policy

1. Code Comments
	•	All source code MUST contain bilingual comments (English and Chinese).
	•	When modifying existing code, all related comments MUST be reviewed and updated to remain accurate in both languages.
	•	No code changes are considered complete if the corresponding bilingual comments are missing, outdated, or inconsistent.


2. Documentation Language & Structure
	•	All documentation MUST be provided in both English and Chinese.
	•	Use en and zh as explicit language identifiers in filenames, for example:
	•	README-en.md
	•	README-zh.md
	•	English and Chinese documents must be functionally equivalent, covering the same content, structure, and level of detail.


3. Documentation Maintenance (docs/)
	•	After every code generation or code change, the docs/ directory MUST be updated to reflect:
	•	New features or behavior changes
	•	Updated architecture or design decisions
	•	Any newly introduced concepts, APIs, or workflows
	•	The docs/ directory is treated as a first-class knowledge base, enabling long-term maintenance and future extension by both humans and other AI tools.
	•	Code changes without corresponding documentation updates are considered incomplete.


4. Documentation Links
	•	All documentation links MUST use relative paths.
	•	Absolute paths are strictly prohibited, including links to repositories, files, or directories within the same project.


5. Testing Discipline
	•	Existing tests MUST NOT be disabled or bypassed to make the test suite pass.
	•	If code changes invalidate existing tests:
	•	Tests MUST be updated according to their original intent and validation goals
	•	Test logic SHOULD be refined to accurately verify new or modified behavior
	•	All test changes must strengthen or preserve reliability, never weaken it.


6. Change Completeness Checklist

Before considering any code change complete, ensure:
	•	Code logic is updated
	•	Bilingual (EN/ZH) comments are accurate and complete
	•	Corresponding documentation is updated in both -en and -zh versions
	•	docs/ reflects the latest knowledge and design
	•	All documentation links use relative paths
	•	Tests are meaningful, enabled, and aligned with updated behavior