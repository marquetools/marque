<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00151" is-a="NoticeHasCorrespondingDataTwoPossibleTokens">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00151][Warning] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and:
        1. No element without @ism:excludeFromRollup=true() in the document has either the attribute @ism:nonICmarkings containing [LES] 
        or the attribute @ism:cuiBasic containing [LEI]
        AND
        2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [LES]    
        and does not specifiy attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing a non-external LES notice must also have LES data in either nonICmarkings or cuiBasic. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule uses an abstract pattern to consolidate logic.
        If the document is an ISM_USGOV_RESOURCE or a ISM_USCUIONLY_RESOURCE, and any element meets
        ISM_CONTRIBUTES and specifies attribute @ism:noticeType
        with a value containing the token [LES] and does not specifiy attribute @ism:externalNotice with a 
        value of [true], this rule ensures that an element
        meeting ISM_CONTRIBUTES specifies either attribute @ism:nonICmarkings
        with a value containing the token [LES] or attribute @ism:cuiBasic with a value containing the token [LEI].
    </sch:p>
    <sch:param name="ruleId" value="'ISM-ID-00151'"/>
    <sch:param name="attrName" value="'nonICmarkings or cuiBasic'"/>
    <sch:param name="dataType1" value="'LES'"/>
    <sch:param name="dataType2" value="'LEI'"/>
    <sch:param name="noticeType" value="'LES'"/>
    <sch:param name="dataTokenList" value="($partNonICmarkings_tok,$partCuiBasic_tok)"/>
</sch:pattern>