<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00488" is-a="NoticeHasCorrespondingCUIData">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00488][Error] If ISM_USCUIONLY_RESOURCE, and:
        1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:cuiSpecified containing [FISA]
        AND
        2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [FISA]
        and does not specifiy attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA CUI-ONLY documents containing a non-external FISA notice must also have FISA data 
        as a CUI Specified Category Marking. 
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic.
		If the document is an ISM_USCUIONLY_RESOURCE, and any element meets
		ISM_CONTRIBUTES and specifies attribute @ism:noticeType
		with a value containing the token [FISA] and does not specifiy attribute @ism:externalNotice with a 
		value of [true], this rule ensures that an element
		meeting ISM_CONTRIBUTES specifies attribute @ism:cuiSpecified
		with a value containing the token [FISA].
	</sch:p>
	<sch:param name="ruleId" value="'ISM-ID-00488'"/>
	<sch:param name="attrName" value="'cuiSpecified'"/>
    <sch:param name="dataType" value="'FISA'"/>
    <sch:param name="noticeType" value="'FISA'"/>
    <sch:param name="dataTokenList" value="$partCuiSpecified_tok"/>
</sch:pattern>