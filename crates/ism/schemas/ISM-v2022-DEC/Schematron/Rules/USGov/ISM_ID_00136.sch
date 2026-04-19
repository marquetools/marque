<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00136" is-a="NoticeHasCorrespondingData">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00136][Error] If ISM_USGOV_RESOURCE and:
        1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:atomicEnergyMarkings
        containing [FRD]
        AND
        2. Any element without ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType 
        containing [FRD]
        and does not specifiy attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing a non-external FRD notice must also have FRD data.
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic.
		If the document is an ISM_USGOV_RESOURCE and any element meets
		ISM_CONTRIBUTES and specifies attribute @ism:noticeType
		with a value containing the token [FRD] and does not specifiy attribute @ism:externalNotice with a 
		value of [true], this rule ensures that an element
		meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
		with a value containing the token [FRD].
	</sch:p>
	<sch:param name="ruleId" value="'ISM-ID-00136'"/>
	<sch:param name="attrName" value="'atomicEnergyMarkings'"/>
    <sch:param name="dataType" value="'FRD'"/>
    <sch:param name="noticeType" value="'FRD'"/>
    <sch:param name="dataTokenList" value="$partAtomicEnergyMarkings_tok"/>
</sch:pattern>