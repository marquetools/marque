<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00238">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00238][Error] If ISM_USDOD_RESOURCE, if any element specifies
    	attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
    	[DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
    	then an element in the document must specify attribute @ism:pocType with
    	the same value as attribute @ism:noticeType.
        
        Human Readable: DoD distribution statements B, C, D, E, and F all 
        require a corresponding point of contact.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USDOD_RESOURCE, for each element which has 
    	attribute @ism:noticeType specified with a value containing the token
        [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F], 
        this rule ensures that some element in the document 
        specifies attribute @ism:pocType with the same value as @ism:noticeType.
    </sch:p>
    <sch:rule id="ISM-ID-00238-R1" context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]">
        <sch:let name="foundNoticeTokens" value="for $noticeToken in tokenize(normalize-space(string(@ism:noticeType)), ' ') return if(matches($noticeToken, '^DoD-Dist-[BCDEF]')) then $noticeToken else null"/>
        <sch:assert test="every $noticeToken in $foundNoticeTokens satisfies index-of($partPocType_tok, $noticeToken)&gt;0" flag="error" role="error"> 
            [ISM-ID-00238][Error] If ISM_USDOD_RESOURCE, if any element specifies
            attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
            [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
            then an element in the document must specify attribute @ism:pocType with
            the same value as attribute @ism:noticeType.
            
            Human Readable: DoD distribution statements B, C, D, E, and F all 
            require a corresponding point of contact.
        </sch:assert>
    </sch:rule>
</sch:pattern>