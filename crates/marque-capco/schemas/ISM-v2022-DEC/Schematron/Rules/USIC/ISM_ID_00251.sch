<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00251">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00251][Error] If ISM_USIC_RESOURCE, then attribute @ism:noticeType must not be specified with a value of [COMSEC]. 
        
        Human Readable: COMSEC notices are not valid for US IC documents.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If ISM_USIC_RESOURCE, for each element which has attribute @ism:noticeType specified, this rule ensures 
    	that attribute @ism:noticeType is not specified with a value containing token [COMSEC].
    </sch:p>
	  <sch:rule id="ISM-ID-00251-R1" context="*[$ISM_USIC_RESOURCE and @ism:noticeType]">
        <sch:assert test="not(util:containsAnyTokenMatching(@ism:noticeType, 'COMSEC'))" flag="error" role="error">
            [ISM-ID-00251][Error] If ISM_USIC_RESOURCE, then attribute @ism:noticeType must not be specified with a value of [COMSEC]. 
            
            Human Readable: COMSEC notices are not valid for US IC documents.
        </sch:assert>
    </sch:rule>
</sch:pattern>