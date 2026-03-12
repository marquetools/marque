<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00153" is-a="NoticeHasCorrespondingData">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00153][Error] If ISM_USGOV_RESOURCE and:
        1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:nonICmarkings containing [LES-NF]
        AND
        2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [LES-NF].
        and does not specifiy attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing an LES-NF notice must also have LES-NF data. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule uses an abstract pattern to consolidate logic.
        If the document is an ISM_USGOV_RESOURCE and any element meets
        ISM_CONTRIBUTES and specifies attribute @ism:noticeType
        with a value containing the token [LES-NF] and does not specifiy attribute @ism:externalNotice with a 
        value of [true], this rule ensures that an element
        meeting ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings
        with a value containing the token [LES-NF].
    </sch:p>
    <sch:param name="ruleId" value="'ISM-ID-00153'"/>
    <sch:param name="attrName" value="'nonICmarkings'"/>
    <sch:param name="dataType" value="'LES-NF'"/>
    <sch:param name="noticeType" value="'LES-NF'"/>
    <sch:param name="dataTokenList" value="$partNonICmarkings_tok"/>
</sch:pattern>