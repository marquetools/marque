<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00138" is-a="NoticeHasCorrespondingDataUnclassDoc">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00138][Error] If ISM_USGOV_RESOURCE and:
        1. No element without @ism:excludeFromRollup=true() in the document has the attribute nonICmarkings containing [DS]
        AND
        2. Any element without @ism:excludeFromRollup=true() in the document has the attribute noticeType containing [DS]
        and does not specifiy attribute @ism:externalNotice with a value of [true].
        AND
        3. $ISM_RESOURCE_ELEMENT is unclassified.
        
        Human Readable: Unclassified USA documents containing a non-external DS notice must also have DS data. 
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic.
		If the document is an ISM_USGOV_RESOURCE and any element meets
		ISM_CONTRIBUTES and specifies attribute @ism:noticeType
		with a value containing the token [DS] and does not specifiy attribute @ism:externalNotice with a 
		value of [true], this rule ensures that an element
		meeting ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings
		with a value containing the token [DS].
		Classified USA documents containing a non-external DS notice must also have DS data however 
		that is not currently testable since classified documents do not have to have DS at the document level.
	</sch:p>
	  <sch:param name="ruleId" value="'ISM-ID-00138'"/>
	  <sch:param name="attrName" value="'nonICmarkings'"/>
	  <sch:param name="dataType" value="'DS'"/>
	  <sch:param name="dataTokenList" value="$partNonICmarkings_tok"/>
</sch:pattern>