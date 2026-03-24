---
type: monthly-retrospective
month: {{date:YYYY-MM}}
tags: [monthly, retrospective]
created: {{date}}
---

# 월간 회고: {{date:YYYY년 MM월}}

## 📅 이번 달 개요
**기간**: {{date:YYYY-MM-01}} ~ {{date:YYYY-MM-DD}}

## 🎯 월간 목표 달성도
### 계획했던 목표
- [ ] 
- [ ] 
- [ ] 

### 실제 달성
- 

## ✅ 완료한 프로젝트
```dataview
LIST
FROM "10-Projects"
WHERE status = "completed" 
AND contains(string(completed), "{{date:YYYY-MM}}")
```

## 📈 월간 성장
### 새로 배운 기술/지식
- 

### 읽은 책/자료
- 

### 완성한 작품/결과물
- 

## 📊 월간 통계

### 노트 생성 통계
```dataview
TABLE length(rows) as "개수"
FROM ""
WHERE created >= date("{{date:YYYY-MM-01}}")
AND created <= date("{{date:YYYY-MM-31}}")
GROUP BY type
```

### 활동 분석
- **총 노트 생성**: 
- **일평균 노트**: 
- **가장 활발한 날**: 
- **주요 활동 영역**: 

## 💡 월간 핵심 인사이트
1. 
2. 
3. 
4. 
5. 

## 🔄 회고

### 이번 달 잘한 점 (Keep)
- 

### 문제점 (Problem)
- 

### 시도할 것 (Try)
- 

### 중단할 것 (Stop)
- 

## 💰 재정 리뷰 (선택)
- **수입**: 
- **지출**: 
- **저축**: 
- **투자**: 

## 🏃 건강 & 습관
### 운동
- 

### 수면
- 평균 수면 시간: 
- 수면 품질: 

### 습관 추적
- [ ] 
- [ ] 

## 🎯 다음 달 계획

### 우선순위 Top 3
1. 
2. 
3. 

### 프로젝트 계획
- 

### 학습 목표
- 

### 개인 목표
- 

## 📸 이번 달 하이라이트
> 이번 달 가장 기억에 남는 순간이나 성취

## 🔗 관련 문서
- [[{{date-1M:YYYY-MM}}|지난 달 회고]]
- [[{{date+1M:YYYY-MM}}|다음 달 회고]]
- [[52.01-Annual-Goals/{{date:YYYY}}년 목표]]

---
**회고 작성일**: {{date:YYYY-MM-DD HH:mm}}
**다음 회고**: {{date+1M:YYYY-MM-01}}